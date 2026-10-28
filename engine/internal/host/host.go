package host

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"sync"
	"time"

	"tropa.local/probe/internal/wire"
)

type ProbeFunc func(context.Context, wire.ProbeRequest, chan<- wire.Candidate) error

type Options struct {
	BuildID           string
	Probe             ProbeFunc
	ParameterSets     []wire.ParameterSet
	UnavailableTuning []wire.UnavailableTuning
}

var ErrProtocol = errors.New("worker protocol failure")
var ErrOutput = errors.New("worker output failure")

type job struct {
	id       string
	cancel   context.CancelFunc
	sequence uint64
}

type server struct {
	ctx      context.Context
	options  Options
	searchID string
	events   chan []byte
	mutex    sync.Mutex
	active   map[string]*job
	seen     map[string]bool
	jobs     sync.WaitGroup
}

// Serve owns its pipe handles. Closing them is required to interrupt blocked I/O on cancellation.
func Serve(parent context.Context, input io.ReadCloser, output io.WriteCloser, options Options) error {
	if options.BuildID == "" || options.Probe == nil {
		return errors.New("invalid worker configuration")
	}
	ctx, cancel := context.WithCancel(parent)
	defer cancel()
	closed := make(chan struct{})
	go func() { <-ctx.Done(); input.Close(); output.Close(); close(closed) }()
	defer func() { cancel(); <-closed }()
	h := &server{ctx: ctx, options: options, events: make(chan []byte, 128), active: make(map[string]*job), seen: make(map[string]bool)}
	written := make(chan error, 1)
	go func() {
		for data := range h.events {
			if err := wire.WriteFrame(output, data); err != nil {
				written <- ErrOutput
				cancel()
				return
			}
		}
		written <- nil
	}()
	graceful, err := h.readCommands(input)
	if !graceful {
		cancel()
	}
	h.mutex.Lock()
	for _, job := range h.active {
		job.cancel()
	}
	h.mutex.Unlock()
	// A stalled reader cannot keep a cooperative worker alive indefinitely during shutdown.
	deadline := time.AfterFunc(2*time.Second, cancel)
	defer deadline.Stop()
	h.jobs.Wait()
	close(h.events)
	writeErr := <-written
	if err != nil {
		return err
	}
	if writeErr != nil {
		return writeErr
	}
	if parent.Err() != nil {
		return parent.Err()
	}
	if ctx.Err() != nil {
		return ErrOutput
	}
	return nil
}

func (h *server) readCommands(input io.Reader) (bool, error) {
	hello := false
	for {
		data, err := wire.ReadFrame(input)
		if errors.Is(err, io.EOF) {
			return true, nil
		}
		if err != nil {
			return false, ErrProtocol
		}
		command, err := wire.DecodeCommand(data)
		if err != nil {
			return false, ErrProtocol
		}
		if !hello {
			if command.Type != "hello" {
				return false, ErrProtocol
			}
			hello = true
			h.searchID = command.SearchID
			h.seen[command.JobID] = true
			state := &job{id: command.JobID}
			tuning := make([]string, 0, len(h.options.ParameterSets))
			for _, profile := range h.options.ParameterSets {
				tuning = append(tuning, profile.Name)
			}
			if err := h.send(h.ctx, state, "hello", wire.Hello{BuildID: h.options.BuildID, Transports: []wire.Transport{wire.WireGuard, wire.AWG, wire.MasqueH3, wire.MasqueH2}, Tuning: tuning, ParameterSets: h.options.ParameterSets, UnavailableTuning: h.options.UnavailableTuning}); err != nil {
				return false, err
			}
			continue
		}
		if command.SearchID != h.searchID {
			return false, ErrProtocol
		}
		switch command.Type {
		case "probe":
			request, err := wire.DecodeProbeRequest(command.Payload)
			if err != nil {
				return false, ErrProtocol
			}
			h.mutex.Lock()
			if h.seen[command.JobID] {
				h.mutex.Unlock()
				return false, ErrProtocol
			}
			h.seen[command.JobID] = true
			if len(h.active) >= 2 {
				h.mutex.Unlock()
				data, err := h.encode(&job{id: command.JobID}, "error", wire.Failure{Stage: "scheduler", Reason: "busy", Retryable: true})
				if err != nil {
					return false, err
				}
				select {
				case h.events <- data:
				default:
					return false, ErrOutput
				}
				continue
			}
			ctx, cancel := context.WithCancel(h.ctx)
			state := &job{id: command.JobID, cancel: cancel}
			h.active[state.id] = state
			h.jobs.Add(1)
			h.mutex.Unlock()
			go h.runJob(ctx, state, request)
		case "cancel":
			h.mutex.Lock()
			state := h.active[command.JobID]
			h.mutex.Unlock()
			if state != nil {
				state.cancel()
			}
		case "shutdown":
			return true, nil
		default:
			return false, ErrProtocol
		}
	}
}

func (h *server) runJob(ctx context.Context, state *job, request wire.ProbeRequest) {
	defer func() { state.cancel(); h.mutex.Lock(); delete(h.active, state.id); h.mutex.Unlock(); h.jobs.Done() }()
	if h.send(h.ctx, state, "accepted", struct{}{}) != nil {
		return
	}
	results := make(chan wire.Candidate)
	done := make(chan error, 1)
	go func() {
		defer func() {
			if recover() != nil {
				done <- errors.New("probe panic")
			}
		}()
		done <- h.options.Probe(ctx, request, results)
	}()
	completed := uint32(0)
	var probeError error
reading:
	for {
		select {
		case <-ctx.Done():
			probeError = <-done
			break reading
		case probeError = <-done:
			break reading
		case result := <-results:
			if h.send(ctx, state, "candidate", result) != nil {
				state.cancel()
				probeError = <-done
				break reading
			}
			completed++
			if h.send(ctx, state, "progress", wire.Progress{Completed: completed, Total: uint32(len(request.Targets))}) != nil {
				state.cancel()
				probeError = <-done
				break reading
			}
		}
	}
	// A terminal frame gives Rust credit to submit the next job.
	h.mutex.Lock()
	delete(h.active, state.id)
	h.mutex.Unlock()
	if ctx.Err() != nil {
		h.send(h.ctx, state, "jobFinished", wire.JobFinished{Outcome: "cancelled"})
	} else if probeError != nil {
		h.send(h.ctx, state, "error", wire.Failure{Stage: "probe", Reason: "probeFailed", Retryable: false})
	} else {
		h.send(h.ctx, state, "jobFinished", wire.JobFinished{Outcome: "completed"})
	}
}

func (h *server) encode(state *job, kind string, payload any) ([]byte, error) {
	raw, err := json.Marshal(payload)
	if err != nil {
		return nil, ErrOutput
	}
	data, err := json.Marshal(wire.Event{Version: wire.Version, SearchID: h.searchID, JobID: state.id, Sequence: state.sequence + 1, Type: kind, Payload: raw})
	if err != nil {
		return nil, ErrOutput
	}
	return data, nil
}

func (h *server) send(ctx context.Context, state *job, kind string, payload any) error {
	data, err := h.encode(state, kind, payload)
	if err != nil {
		return err
	}
	select {
	case h.events <- data:
		state.sequence++
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}
