package probe

import (
	"context"
	"errors"
	"net"
	"net/netip"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/device"
	"github.com/amnezia-vpn/amneziawg-go/tun/netstack"
	"tropa.local/probe/internal/wire"
)

type wgSession struct {
	device    *device.Device
	network   *netstack.Net
	bind      *udpBind
	closeOnce sync.Once
	cancel    context.CancelFunc
	done      chan struct{}
}

func newWG(ctx context.Context, request wire.ProbeRequest, target string) (*wgSession, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	configuration, err := wgConfiguration(request, target)
	if err != nil {
		return nil, err
	}
	identity := request.Identity.WireGuard
	addresses := []netip.Addr{netip.MustParseAddr(identity.IPv4)}
	if identity.IPv6 != "" {
		addresses = append(addresses, netip.MustParseAddr(identity.IPv6))
	}
	tun, network, err := netstack.CreateNetTUN(addresses, []netip.Addr{netip.MustParseAddr("1.1.1.1")}, 1280)
	if err != nil {
		return nil, errors.New("WG userspace stack creation failed")
	}
	bind := &udpBind{reserved: identity.Reserved}
	dev := device.NewDevice(tun, bind, device.NewLogger(device.LogLevelSilent, ""))
	if err := dev.IpcSet(configuration); err != nil {
		dev.Close()
		return nil, errors.New("WG device configuration failed")
	}
	if err := dev.Up(); err != nil {
		dev.Close()
		return nil, errors.New("WG device activation failed")
	}
	lifetime, cancel := context.WithCancel(ctx)
	session := &wgSession{device: dev, network: network, bind: bind, cancel: cancel, done: make(chan struct{})}
	go func() {
		<-lifetime.Done()
		session.Close()
	}()
	return session, nil
}

func (s *wgSession) Close() {
	s.closeOnce.Do(func() {
		s.cancel()
		s.device.Close()
		close(s.done)
	})
}

func (s *wgSession) DialContext(ctx context.Context, network, address string) (net.Conn, error) {
	return s.network.DialContext(ctx, network, address)
}

func (s *wgSession) Network() *netstack.Net { return s.network }

func (s *wgSession) Handshake(ctx context.Context) bool {
	trigger, err := s.network.DialUDPAddrPort(netip.AddrPort{}, netip.MustParseAddrPort("1.1.1.1:53"))
	if err != nil {
		return false
	}
	defer trigger.Close()
	if _, err := trigger.Write(make([]byte, 12)); err != nil {
		return false
	}
	ticker := time.NewTicker(10 * time.Millisecond)
	defer ticker.Stop()
	for {
		configuration, err := s.device.IpcGet()
		if err != nil {
			return false
		}
		for line := range strings.SplitSeq(configuration, "\n") {
			if value, ok := strings.CutPrefix(line, "last_handshake_time_sec="); ok {
				seconds, err := strconv.ParseInt(value, 10, 64)
				if err == nil && seconds > 0 {
					return true
				}
			}
		}
		select {
		case <-ctx.Done():
			return false
		case <-s.done:
			return false
		case <-ticker.C:
		}
	}
}
