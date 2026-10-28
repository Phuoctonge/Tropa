package probe

import (
	"context"
	"errors"
	"net"
	"net/netip"
	"sync"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/tun"
	"github.com/amnezia-vpn/amneziawg-go/tun/netstack"
	"tropa.local/probe/internal/wire"
)

type masqueSession struct {
	device  tun.Device
	network *netstack.Net
	link    *masqueLink
	cancel  context.CancelFunc
	pumps   sync.WaitGroup
	once    sync.Once
	done    chan struct{}
}

func newMasque(ctx context.Context, request wire.ProbeRequest, target string) (*masqueSession, bool, error) {
	if err := ctx.Err(); err != nil {
		return nil, false, err
	}
	configuration, err := masqueTLS(request)
	if err != nil {
		return nil, false, err
	}
	endpoint, err := netip.ParseAddrPort(target)
	if err != nil || endpoint.Port() == 0 || endpoint.Addr().Is6() != request.IPv6 {
		return nil, false, errors.New("invalid MASQUE endpoint")
	}
	timeout := time.Duration(request.TimeoutMS) * time.Millisecond
	link, err := dialMasqueWithLifetime(ctx, timeout, func(lifetime context.Context) (*masqueLink, error) {
		return dialMasque(lifetime, configuration, endpoint, request.Transport == wire.MasqueH2, timeout)
	})
	if err != nil {
		return nil, false, nil
	}
	identity := request.Identity.Masque
	addresses := []netip.Addr{netip.MustParseAddr(identity.IPv4)}
	if identity.IPv6 != "" {
		addresses = append(addresses, netip.MustParseAddr(identity.IPv6))
	}
	device, network, err := netstack.CreateNetTUN(addresses, []netip.Addr{netip.MustParseAddr("1.1.1.1")}, 1280)
	if err != nil {
		link.Close()
		return nil, false, errors.New("MASQUE userspace stack creation failed")
	}
	lifetime, cancel := context.WithCancel(ctx)
	session := &masqueSession{device: device, network: network, link: link, cancel: cancel, done: make(chan struct{})}
	session.pumps.Add(2)
	go session.pumpOut()
	go session.pumpIn()
	go func() { <-lifetime.Done(); session.Close() }()
	return session, true, nil
}

func (s *masqueSession) pumpOut() {
	defer s.pumps.Done()
	defer s.cancel()
	buffer := make([]byte, 1408)
	sizes := make([]int, 1)
	for {
		if _, err := s.device.Read([][]byte{buffer}, sizes, 0); err != nil {
			return
		}
		icmp, err := s.link.packets.WritePacket(buffer[:sizes[0]])
		if err != nil {
			return
		}
		if len(icmp) > 0 {
			if _, err := s.device.Write([][]byte{icmp}, 0); err != nil {
				return
			}
		}
	}
}

func (s *masqueSession) pumpIn() {
	defer s.pumps.Done()
	defer s.cancel()
	buffer := make([]byte, 1408)
	for {
		n, err := s.link.packets.ReadPacket(buffer, true)
		if err != nil {
			return
		}
		if _, err := s.device.Write([][]byte{buffer[:n]}, 0); err != nil {
			return
		}
	}
}

func (s *masqueSession) Close() {
	s.once.Do(func() { s.cancel(); s.link.Close(); s.device.Close(); s.pumps.Wait(); close(s.done) })
}

func (s *masqueSession) DialContext(ctx context.Context, network, address string) (net.Conn, error) {
	return s.network.DialContext(ctx, network, address)
}

func (s *masqueSession) Network() *netstack.Net { return s.network }
