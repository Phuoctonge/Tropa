package probe

import (
	"context"
	"errors"
	"net"
	"testing"
	"time"

	"tropa.local/probe/internal/wire"
)

func TestProbeCancellationUnblocksResultDelivery(t *testing.T) {
	remote, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	defer remote.Close()
	request := wgRequest(1)
	request.Targets = []string{remote.LocalAddr().String()}
	request.TimeoutMS = 20
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	results := make(chan wire.Candidate)
	done := make(chan error, 1)
	go func() { done <- Probe(ctx, request, results) }()
	// A completed no-handshake candidate is waiting for a reader here.
	time.Sleep(100 * time.Millisecond)
	cancel()
	select {
	case err := <-done:
		if !errors.Is(err, context.Canceled) {
			t.Fatal("cancellation was not returned:", err)
		}
	case <-time.After(time.Second):
		t.Fatal("blocked result delivery prevented cleanup")
	}
}

func TestProbeFailedHandshakeDoesNotInventDataResults(t *testing.T) {
	remote, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	defer remote.Close()
	request := wgRequest(1)
	request.Targets = []string{remote.LocalAddr().String()}
	request.TimeoutMS = 10
	results := make(chan wire.Candidate, 1)
	if err := Probe(context.Background(), request, results); err != nil {
		t.Fatal(err)
	}
	result := <-results
	if result.Stages.Handshake == nil || *result.Stages.Handshake || result.Stages.HTTP != nil || result.Stages.HTTPS != nil || result.Stages.UDP != nil {
		t.Fatal("failed handshake was confused with a data measurement")
	}
	if result.TunnelPingMS != nil || result.LossPercent != nil || result.Samples != 0 {
		t.Fatal("unperformed measurements were populated")
	}
}

func TestDeliveredParametersCannotMutateRemainingTargets(t *testing.T) {
	remote, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	defer remote.Close()
	request := wgRequest(1)
	request.Targets = []string{remote.LocalAddr().String(), remote.LocalAddr().String()}
	request.TimeoutMS = 10
	results := make(chan wire.Candidate)
	done := make(chan error, 1)
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	go func() { done <- Probe(ctx, request, results) }()
	first := <-results
	first.Parameters.AWG.I1 = "mutated-result"
	second := <-results
	if second.Parameters.AWG.I1 != "<b 0x0102>" {
		t.Error("delivered result shared mutable parameters with remaining work")
	}
	if err := <-done; err != nil {
		t.Fatal(err)
	}
}
