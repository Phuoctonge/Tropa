package probe

import (
	"context"
	"crypto/tls"
	"errors"
	"net"
	"net/http"
	"net/netip"
	"sync"
	"time"

	connectip "github.com/Diniboy1123/connect-ip-go"
	"github.com/Diniboy1123/usque/api"
	"github.com/quic-go/quic-go"
	"github.com/yosida95/uritemplate/v3"
	"golang.org/x/net/http2"
)

const masqueURI = "https://cloudflareaccess.com"

type ipPackets interface {
	ReadPacket([]byte, bool) (int, error)
	WritePacket([]byte) ([]byte, error)
}

type masqueLink struct {
	packets        ipPackets
	closeTransport func()
	cancel         context.CancelFunc
	once           sync.Once
}

func (link *masqueLink) Close() {
	link.once.Do(func() {
		if link.cancel != nil {
			link.cancel()
		}
		if link.closeTransport != nil {
			link.closeTransport()
		}
	})
}

func dialMasqueWithLifetime(parent context.Context, timeout time.Duration, dial func(context.Context) (*masqueLink, error)) (*masqueLink, error) {
	lifetime, cancel := context.WithCancel(parent)
	timer := time.AfterFunc(timeout, cancel)
	link, err := dial(lifetime)
	stopped := timer.Stop()
	if link != nil {
		link.cancel = cancel
	}
	if err != nil || !stopped || lifetime.Err() != nil || link == nil {
		cancel()
		if link != nil {
			link.Close()
		}
		return nil, errors.New("MASQUE connection failed")
	}
	context.AfterFunc(lifetime, link.Close)
	return link, nil
}

func dialMasque(ctx context.Context, configuration *tls.Config, target netip.AddrPort, h2 bool, timeout time.Duration) (*masqueLink, error) {
	if h2 {
		return dialMasqueH2(ctx, configuration, target)
	}
	udp, transport, connection, _, err := api.ConnectTunnel(ctx, configuration, &quic.Config{
		EnableDatagrams: true, KeepAlivePeriod: 30 * time.Second, HandshakeIdleTimeout: timeout, MaxIdleTimeout: 5 * time.Second,
	}, masqueURI, net.UDPAddrFromAddrPort(target), false)
	link := &masqueLink{packets: connection, closeTransport: func() {
		if connection != nil {
			connection.Close()
		}
		if transport != nil {
			transport.Close()
		}
		if udp != nil {
			udp.Close()
		}
	}}
	if err == nil && connection == nil {
		err = errors.New("missing MASQUE connection")
	}
	return link, err
}

func dialMasqueH2(ctx context.Context, configuration *tls.Config, target netip.AddrPort) (*masqueLink, error) {
	tlsConfiguration := configuration.Clone()
	tlsConfiguration.NextProtos = []string{"h2"}
	var mutex sync.Mutex
	var tcp net.Conn
	closed := false
	transport := &http2.Transport{DialTLSContext: func(dialContext context.Context, _, _ string, _ *tls.Config) (net.Conn, error) {
		dialer := tls.Dialer{NetDialer: &net.Dialer{}, Config: tlsConfiguration}
		connection, err := dialer.DialContext(dialContext, "tcp", target.String())
		if err != nil {
			return nil, err
		}
		mutex.Lock()
		defer mutex.Unlock()
		if closed || ctx.Err() != nil {
			connection.Close()
			return nil, context.Canceled
		}
		tcp = connection
		return connection, nil
	}}
	client := &http.Client{Transport: transport, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	connection, _, err := connectip.DialH2(ctx, client, uritemplate.MustNew(masqueURI), http.Header{
		"User-Agent": []string{""}, "Cf-Connect-Proto": []string{"cf-connect-ip"}, "Pq-Enabled": []string{"false"},
	})
	return &masqueLink{packets: connection, closeTransport: func() {
		mutex.Lock()
		closed = true
		if tcp != nil {
			tcp.Close()
		}
		mutex.Unlock()
		if connection != nil {
			connection.Close()
		}
		transport.CloseIdleConnections()
	}}, err
}
