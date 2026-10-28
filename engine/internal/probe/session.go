package probe

import (
	"context"
	"errors"
	"github.com/amnezia-vpn/amneziawg-go/tun/netstack"
	"net"
	"net/netip"
	"slices"
	"sync"
	"time"

	"tropa.local/probe/internal/wire"
)

// Probe closes each target's resources before delivering its result. The caller owns results.
func Probe(ctx context.Context, request wire.ProbeRequest, results chan<- wire.Candidate) error {
	if err := request.Validate(); err != nil {
		return err
	}
	request = snapshotRequest(request)
	switch request.Transport {
	case wire.WireGuard, wire.AWG:
		if _, err := wgConfiguration(request, request.Targets[0]); err != nil {
			return err
		}
	case wire.MasqueH2, wire.MasqueH3:
		if _, err := masqueTLS(request); err != nil {
			return err
		}
	default:
		return errors.New("transport adapter unavailable")
	}
	lifetime, cancel := context.WithCancelCause(ctx)
	defer cancel(nil)
	queue := make(chan string, len(request.Targets))
	for _, target := range request.Targets {
		queue <- target
	}
	close(queue)
	var workers sync.WaitGroup
	for range min(int(request.TunnelLimit), len(request.Targets)) {
		workers.Go(func() {
			for {
				select {
				case <-lifetime.Done():
					return
				case target, ok := <-queue:
					if !ok || lifetime.Err() != nil {
						return
					}
					result, err := probeTarget(lifetime, request, target)
					if err != nil {
						cancel(err)
						return
					}
					select {
					case results <- result:
					case <-lifetime.Done():
						return
					}
				}
			}
		})
	}
	workers.Wait()
	return context.Cause(lifetime)
}

type dataSession interface {
	DialContext(context.Context, string, string) (net.Conn, error)
	Network() *netstack.Net
	Close()
}

func probeTarget(ctx context.Context, request wire.ProbeRequest, target string) (wire.Candidate, error) {
	timeout := time.Duration(request.TimeoutMS) * time.Millisecond
	var session dataSession
	var handshake bool
	switch request.Transport {
	case wire.WireGuard, wire.AWG:
		wg, err := newWG(ctx, request, target)
		if err != nil {
			return wire.Candidate{}, err
		}
		session = wg
		stage, cancel := context.WithTimeout(ctx, timeout)
		handshake = wg.Handshake(stage)
		cancel()
	case wire.MasqueH2, wire.MasqueH3:
		masque, connected, err := newMasque(ctx, request, target)
		if err != nil {
			return wire.Candidate{}, err
		}
		handshake = connected
		if masque != nil {
			session = masque
		}
	}
	if session != nil {
		defer session.Close()
	}
	result := wire.Candidate{}
	if handshake && ctx.Err() == nil {
		client := probeHTTPClient(session.DialContext)
		defer client.CloseIdleConnections()
		result = measureData(ctx, client, traceTargets{
			HTTP:  "http://www.cloudflare.com/cdn-cgi/trace",
			HTTPS: "https://www.cloudflare.com/cdn-cgi/trace",
		}, timeout)
		if result.Stages.HTTPS != nil && *result.Stages.HTTPS {
			ping := measurePing(ctx, session.Network(), netip.MustParseAddr("1.1.1.1"), timeout)
			result.TunnelPingMS, result.LossPercent, result.JitterMS = ping.TunnelPingMS, ping.LossPercent, ping.JitterMS
			result.Samples, result.Method = ping.Samples, ping.Method
		}
	}
	result.Endpoint = target
	result.Transport = request.Transport
	result.Parameters = snapshotParameters(request.Parameters)
	result.Stages.Handshake = &handshake
	return result, nil
}

func snapshotRequest(request wire.ProbeRequest) wire.ProbeRequest {
	request.Targets = slices.Clone(request.Targets)
	if request.Identity.WireGuard != nil {
		identity := *request.Identity.WireGuard
		request.Identity.WireGuard = &identity
	}
	if request.Identity.Masque != nil {
		identity := *request.Identity.Masque
		request.Identity.Masque = &identity
	}
	request.Parameters = snapshotParameters(request.Parameters)
	return request
}

func snapshotParameters(parameters wire.Parameters) wire.Parameters {
	if parameters.AWG != nil {
		awg := *parameters.AWG
		parameters.AWG = &awg
	}
	return parameters
}
