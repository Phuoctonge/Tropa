package host

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"testing"
	"time"

	"tropa.local/probe/internal/probe"
	"tropa.local/probe/internal/wire"
)

type client struct {
	input  *io.PipeWriter
	output *io.PipeReader
	done   chan error
}

func startHost(t *testing.T, run ProbeFunc) *client {
	t.Helper()
	input, commands := io.Pipe()
	events, output := io.Pipe()
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	c := &client{commands, events, make(chan error, 1)}
	profiles, unavailable := probe.TuningProfiles()
	go func() {
		c.done <- Serve(ctx, input, output, Options{BuildID: "test-build", Probe: run, ParameterSets: profiles, UnavailableTuning: unavailable})
		close(c.done)
	}()
	t.Cleanup(func() {
		cancel()
		commands.Close()
		events.Close()
		select {
		case <-c.done:
		case <-time.After(time.Second):
			t.Error("host did not stop")
		}
	})
	return c
}

func (c *client) command(t *testing.T, kind, id string, payload any) {
	t.Helper()
	raw, err := json.Marshal(payload)
	if err != nil {
		t.Fatal(err)
	}
	data, _ := json.Marshal(wire.Command{Version: wire.Version, SearchID: "search", JobID: id, Type: kind, Payload: raw})
	if err := wire.WriteFrame(c.input, data); err != nil {
		t.Fatal("command write failed:", err)
	}
}

func (c *client) event(t *testing.T) wire.Event {
	t.Helper()
	data, err := wire.ReadFrame(c.output)
	if err != nil {
		t.Fatal("event read failed:", err)
	}
	if bytes.Contains(data, []byte("privateKey")) || bytes.Contains(data, []byte("identity")) {
		t.Fatal("identity leaked into event")
	}
	var event wire.Event
	if err := json.Unmarshal(data, &event); err != nil {
		t.Fatal(err)
	}
	return event
}

func request(port string) wire.ProbeRequest {
	key := base64.StdEncoding.EncodeToString(bytes.Repeat([]byte{7}, 32))
	return wire.ProbeRequest{Transport: wire.WireGuard, Targets: []string{"127.0.0.1:" + port}, TimeoutMS: 100, TunnelLimit: 4,
		Identity: wire.Identity{WireGuard: &wire.WGIdentity{PrivateKey: key, PeerPublicKey: key, IPv4: "172.16.0.2"}}}
}

func hello(t *testing.T, c *client) {
	t.Helper()
	c.command(t, "hello", "control", struct{}{})
	event := c.event(t)
	if event.Type != "hello" || event.Sequence != 1 {
		t.Fatal("hello handshake missing")
	}
}

func TestHelloProvidesConcreteTuningBeforeAnyIdentity(t *testing.T) {
	c := startHost(t, func(context.Context, wire.ProbeRequest, chan<- wire.Candidate) error {
		t.Error("hello started a probe")
		return nil
	})
	c.command(t, "hello", "control", struct{}{})
	event := c.event(t)
	var payload wire.Hello
	if err := json.Unmarshal(event.Payload, &payload); err != nil {
		t.Fatal(err)
	}
	if len(payload.ParameterSets) == 0 || len(payload.ParameterSets) > 32 || len(payload.UnavailableTuning) != 1 {
		t.Fatal("tuning catalog missing")
	}
	found := false
	for _, profile := range payload.ParameterSets {
		if profile.Name == "awg-dns-google-i1" {
			found = profile.Parameters.AWG != nil && profile.Parameters.AWG.JC == 0 && profile.Parameters.AWG.I1 != ""
		}
	}
	if !found || payload.UnavailableTuning[0].Name != "awg-quic" {
		t.Fatal("concrete I1 or unavailable capability missing")
	}
	c.command(t, "shutdown", "stop", struct{}{})
	if err := <-c.done; err != nil {
		t.Fatal(err)
	}
}

func TestCompletedJobReleasesItsSlotBeforeTerminalDelivery(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	h := &server{ctx: ctx, options: Options{Probe: func(context.Context, wire.ProbeRequest, chan<- wire.Candidate) error { return nil }}, events: make(chan []byte), active: make(map[string]*job)}
	jobCtx, stop := context.WithCancel(ctx)
	state := &job{id: "one", cancel: stop}
	h.active[state.id] = state
	h.jobs.Add(1)
	go h.runJob(jobCtx, state, request("10001"))
	defer func() { cancel(); h.jobs.Wait() }()
	<-h.events
	deadline := time.Now().Add(time.Second)
	for {
		h.mutex.Lock()
		count := len(h.active)
		h.mutex.Unlock()
		if count == 0 {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("completed job still occupies a slot while terminal delivery is blocked")
		}
		time.Sleep(time.Millisecond)
	}
	data := <-h.events
	var event wire.Event
	if err := json.Unmarshal(data, &event); err != nil || event.Type != "jobFinished" {
		t.Fatal("terminal completion missing")
	}
}

func TestStreamingCancelIsTerminalOnlyAfterProbeCleanup(t *testing.T) {
	cleaned := make(chan struct{})
	c := startHost(t, func(ctx context.Context, r wire.ProbeRequest, out chan<- wire.Candidate) error {
		defer close(cleaned)
		select {
		case out <- wire.Candidate{Endpoint: r.Targets[0], Transport: r.Transport, Parameters: r.Parameters}:
		case <-ctx.Done():
			return ctx.Err()
		}
		<-ctx.Done()
		return ctx.Err()
	})
	hello(t, c)
	c.command(t, "probe", "one", request("10001"))
	for _, kind := range []string{"accepted", "candidate", "progress"} {
		if event := c.event(t); event.Type != kind {
			t.Fatal("unexpected event order:", event.Type)
		}
	}
	c.command(t, "cancel", "one", struct{}{})
	terminal := c.event(t)
	if terminal.Type != "jobFinished" || terminal.Sequence != 4 || !bytes.Contains(terminal.Payload, []byte("cancelled")) {
		t.Fatal("cancelled terminal event missing")
	}
	select {
	case <-cleaned:
	default:
		t.Fatal("terminal event preceded probe cleanup")
	}
}

func TestHostRejectsThirdConcurrentJob(t *testing.T) {
	started := make(chan struct{}, 2)
	c := startHost(t, func(ctx context.Context, _ wire.ProbeRequest, _ chan<- wire.Candidate) error {
		started <- struct{}{}
		<-ctx.Done()
		return ctx.Err()
	})
	hello(t, c)
	for _, id := range []string{"one", "two"} {
		c.command(t, "probe", id, request("10001"))
		if c.event(t).Type != "accepted" {
			t.Fatal("job not accepted")
		}
		<-started
	}
	c.command(t, "probe", "three", request("10003"))
	event := c.event(t)
	if event.Type != "error" || event.JobID != "three" || !bytes.Contains(event.Payload, []byte("busy")) {
		t.Fatal("concurrency budget was not enforced")
	}
}

func TestInputEOFCancelsProbeAndDrainsTerminal(t *testing.T) {
	cleaned := make(chan struct{})
	c := startHost(t, func(ctx context.Context, _ wire.ProbeRequest, _ chan<- wire.Candidate) error {
		<-ctx.Done()
		close(cleaned)
		return ctx.Err()
	})
	hello(t, c)
	c.command(t, "probe", "one", request("10001"))
	c.event(t)
	c.input.Close()
	if event := c.event(t); event.Type != "jobFinished" {
		t.Fatal("EOF did not terminate job")
	}
	if _, err := wire.ReadFrame(c.output); err != io.EOF {
		t.Fatal("event stream did not close")
	}
	if err := <-c.done; err != nil {
		t.Fatal("clean EOF failed:", err)
	}
	select {
	case <-cleaned:
	default:
		t.Fatal("EOF left probe alive")
	}
}

func TestSlowReaderDoesNotBlockCancelInput(t *testing.T) {
	cleaned := make(chan struct{})
	c := startHost(t, func(ctx context.Context, r wire.ProbeRequest, out chan<- wire.Candidate) error {
		defer close(cleaned)
		for range 1000 {
			select {
			case out <- wire.Candidate{Endpoint: r.Targets[0], Transport: r.Transport}:
			case <-ctx.Done():
				return ctx.Err()
			}
		}
		return nil
	})
	hello(t, c)
	c.command(t, "probe", "one", request("10001"))
	c.event(t)
	time.Sleep(50 * time.Millisecond)
	c.command(t, "cancel", "one", struct{}{})
	select {
	case <-cleaned:
	case <-time.After(time.Second):
		t.Fatal("output backpressure prevented cancellation")
	}
	c.command(t, "shutdown", "control", struct{}{})
	for {
		_, err := wire.ReadFrame(c.output)
		if err == io.EOF {
			break
		}
		if err != nil {
			t.Fatal(err)
		}
	}
	if err := <-c.done; err != nil {
		t.Fatal("drained shutdown failed:", err)
	}
}

func TestProtocolFailuresDoNotEchoInput(t *testing.T) {
	for _, kind := range []string{"version", "beforeHello", "foreignSearch"} {
		t.Run(kind, func(t *testing.T) {
			c := startHost(t, func(context.Context, wire.ProbeRequest, chan<- wire.Candidate) error {
				t.Error("invalid command launched a probe")
				return nil
			})
			command := wire.Command{Version: wire.Version, SearchID: "search", JobID: "bad", Type: "probe"}
			command.Payload, _ = json.Marshal(request("10001"))
			if kind == "version" {
				command.Version = 2
			}
			if kind == "foreignSearch" {
				hello(t, c)
				command.SearchID = "foreign"
			}
			data, _ := json.Marshal(command)
			if err := wire.WriteFrame(c.input, data); err != nil {
				t.Fatal(err)
			}
			if _, err := wire.ReadFrame(c.output); err != io.EOF {
				t.Fatal("invalid input produced an event")
			}
			if err := <-c.done; !errors.Is(err, ErrProtocol) {
				t.Fatal("invalid input was not rejected as protocol failure")
			}
		})
	}
}

func TestProbeFailureAndPanicHaveSanitizedTerminal(t *testing.T) {
	for _, panicProbe := range []bool{false, true} {
		c := startHost(t, func(context.Context, wire.ProbeRequest, chan<- wire.Candidate) error {
			if panicProbe {
				panic("private-secret-fixture")
			}
			return errors.New("private-secret-fixture")
		})
		hello(t, c)
		c.command(t, "probe", "one", request("10001"))
		c.event(t)
		event := c.event(t)
		if event.Type != "error" || bytes.Contains(event.Payload, []byte("private-secret-fixture")) {
			t.Fatal("raw probe failure escaped into IPC")
		}
	}
}
