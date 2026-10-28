package probe

import (
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"encoding/base64"
	"errors"
	connectip "github.com/Diniboy1123/connect-ip-go"
	"github.com/quic-go/quic-go/http3"
	"math/big"
	"net"
	"net/http"
	"net/http/httptest"
	"net/netip"
	"testing"
	"time"

	"tropa.local/probe/internal/wire"
)

func masqueRequest(t *testing.T) (wire.ProbeRequest, *ecdsa.PrivateKey) {
	t.Helper()
	private, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	peer, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	privateDER, _ := x509.MarshalECPrivateKey(private)
	peerDER, _ := x509.MarshalPKIXPublicKey(&peer.PublicKey)
	return wire.ProbeRequest{Transport: wire.MasqueH2, Targets: []string{"127.0.0.1:443"}, TimeoutMS: 100, TunnelLimit: 1,
		Identity:   wire.Identity{Masque: &wire.ECIdentity{PrivateKeyDER: base64.StdEncoding.EncodeToString(privateDER), PeerPublicKeyDER: base64.StdEncoding.EncodeToString(peerDER), IPv4: "172.16.0.2"}},
		Parameters: wire.Parameters{SNI: "consumer-masque.cloudflareclient.com"}}, peer
}

func TestMasqueTLSRequiresPinnedPeer(t *testing.T) {
	request, peer := masqueRequest(t)
	configuration, err := masqueTLS(request)
	if err != nil {
		t.Fatal(err)
	}
	certificate, err := x509.CreateCertificate(rand.Reader, &x509.Certificate{SerialNumber: big.NewInt(1)}, &x509.Certificate{}, &peer.PublicKey, peer)
	if err != nil {
		t.Fatal(err)
	}
	if configuration.VerifyPeerCertificate == nil {
		t.Fatal("server public key is not verified")
	}
	if err := configuration.VerifyPeerCertificate([][]byte{certificate}, nil); err != nil {
		t.Fatal("enrolled peer rejected")
	}
	if err := configuration.VerifyPeerCertificate(nil, nil); err == nil {
		t.Fatal("missing peer certificate accepted")
	}
	other, _ := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	wrong, _ := x509.CreateCertificate(rand.Reader, &x509.Certificate{SerialNumber: big.NewInt(2)}, &x509.Certificate{}, &other.PublicKey, other)
	if err := configuration.VerifyPeerCertificate([][]byte{wrong}, nil); err == nil {
		t.Fatal("different peer public key accepted")
	}
}

func TestMasqueDialDeadlineDoesNotEndEstablishedStream(t *testing.T) {
	parent, cancel := context.WithCancel(context.Background())
	defer cancel()
	var streamContext context.Context
	closed := make(chan struct{})
	link, err := dialMasqueWithLifetime(parent, 20*time.Millisecond, func(ctx context.Context) (*masqueLink, error) {
		streamContext = ctx
		return &masqueLink{closeTransport: func() { close(closed) }}, nil
	})
	if err != nil {
		t.Fatal(err)
	}
	time.Sleep(50 * time.Millisecond)
	if streamContext.Err() != nil {
		t.Fatal("dial timeout killed an established stream")
	}
	cancel()
	select {
	case <-closed:
	case <-time.After(time.Second):
		t.Fatal("parent cancellation did not close the transport")
	}
	link.Close()
}

func TestMasqueDialTimeoutClosesPartialTransport(t *testing.T) {
	closed := make(chan struct{})
	_, err := dialMasqueWithLifetime(context.Background(), 10*time.Millisecond, func(ctx context.Context) (*masqueLink, error) {
		<-ctx.Done()
		return &masqueLink{closeTransport: func() { close(closed) }}, ctx.Err()
	})
	if err == nil {
		t.Fatal("timed out dial accepted")
	}
	select {
	case <-closed:
	default:
		t.Fatal("partial transport leaked after dial timeout")
	}
}

func TestMasqueSessionCancellationWaitsForPumps(t *testing.T) {
	request, peer := masqueRequest(t)
	certificate, err := x509.CreateCertificate(rand.Reader, &x509.Certificate{SerialNumber: big.NewInt(4)}, &x509.Certificate{}, &peer.PublicKey, peer)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
		w.(http.Flusher).Flush()
		<-r.Context().Done()
	}))
	server.EnableHTTP2 = true
	server.TLS = &tls.Config{Certificates: []tls.Certificate{{Certificate: [][]byte{certificate}, PrivateKey: peer}}}
	server.StartTLS()
	defer server.Close()
	request.TimeoutMS = 1000
	request.Targets = []string{server.Listener.Addr().String()}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	session, connected, err := newMasque(ctx, request, request.Targets[0])
	if err != nil || !connected {
		t.Fatal("local session did not connect:", err)
	}
	cancel()
	select {
	case <-session.done:
	case <-time.After(time.Second):
		t.Fatal("MASQUE cancellation left packet pumps running")
	}
	session.Close()
}

func TestMasqueH2UsesPinnedEndpointAndSurvivesDialDeadline(t *testing.T) {
	t.Setenv("HTTPS_PROXY", "http://127.0.0.1:1")
	request, peer := masqueRequest(t)
	certificate, err := x509.CreateCertificate(rand.Reader, &x509.Certificate{SerialNumber: big.NewInt(3)}, &x509.Certificate{}, &peer.PublicKey, peer)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.ProtoMajor != 2 || r.Method != http.MethodConnect || r.Header.Get("cf-connect-proto") != "cf-connect-ip" {
			w.WriteHeader(400)
			return
		}
		w.WriteHeader(200)
		w.(http.Flusher).Flush()
		buffer := make([]byte, 4096)
		for {
			n, err := r.Body.Read(buffer)
			if n > 0 {
				if _, writeErr := w.Write(buffer[:n]); writeErr != nil {
					return
				}
				w.(http.Flusher).Flush()
			}
			if err != nil {
				return
			}
		}
	}))
	server.EnableHTTP2 = true
	server.TLS = &tls.Config{Certificates: []tls.Certificate{{Certificate: [][]byte{certificate}, PrivateKey: peer}}}
	server.StartTLS()
	defer server.Close()
	configuration, err := masqueTLS(request)
	if err != nil {
		t.Fatal(err)
	}
	target, err := netip.ParseAddrPort(server.Listener.Addr().String())
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	link, err := dialMasqueWithLifetime(ctx, 200*time.Millisecond, func(ctx context.Context) (*masqueLink, error) { return dialMasqueH2(ctx, configuration, target) })
	if err != nil {
		t.Fatal("local H2 CONNECT-IP failed:", err)
	}
	defer link.Close()
	time.Sleep(300 * time.Millisecond)
	packet := make([]byte, 20)
	packet[0] = 0x45
	packet[3] = 20
	packet[8] = 64
	packet[9] = 17
	copy(packet[12:16], []byte{172, 16, 0, 2})
	copy(packet[16:20], []byte{1, 1, 1, 1})
	if _, err := link.packets.WritePacket(packet); err != nil {
		t.Fatal("established H2 stream died at dial deadline:", err)
	}
	buffer := make([]byte, 1500)
	n, err := link.packets.ReadPacket(buffer, true)
	if err != nil || n != 20 || buffer[0] != 0x45 || buffer[8] != 63 {
		t.Fatal("H2 did not carry the IP datagram past its dial deadline")
	}
}

func TestMasqueH3CarriesIPDatagrams(t *testing.T) {
	request, peer := masqueRequest(t)
	request.Transport = wire.MasqueH3
	certificate, err := x509.CreateCertificate(rand.Reader, &x509.Certificate{SerialNumber: big.NewInt(5)}, &x509.Certificate{}, &peer.PublicKey, peer)
	if err != nil {
		t.Fatal(err)
	}
	udp, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	server := &http3.Server{TLSConfig: &tls.Config{Certificates: []tls.Certificate{{Certificate: [][]byte{certificate}, PrivateKey: peer}}}, EnableDatagrams: true,
		Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			connection, err := (&connectip.Proxy{}).Proxy(w, nil)
			if err != nil {
				return
			}
			defer connection.Close()
			buffer := make([]byte, 1500)
			for {
				n, err := connection.ReadPacket(buffer, true)
				if err != nil {
					return
				}
				if _, err := connection.WritePacket(buffer[:n]); err != nil {
					return
				}
			}
		}),
	}
	done := make(chan struct{})
	go func() { server.Serve(udp); close(done) }()
	defer func() {
		server.Close()
		udp.Close()
		select {
		case <-done:
		case <-time.After(time.Second):
			t.Error("local H3 server did not close")
		}
	}()
	configuration, err := masqueTLS(request)
	if err != nil {
		t.Fatal(err)
	}
	target, err := netip.ParseAddrPort(udp.LocalAddr().String())
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	link, err := dialMasqueWithLifetime(ctx, time.Second, func(ctx context.Context) (*masqueLink, error) {
		return dialMasque(ctx, configuration, target, false, time.Second)
	})
	if err != nil {
		t.Fatal("local H3 CONNECT-IP failed:", err)
	}
	defer link.Close()
	packet := make([]byte, 20)
	packet[0] = 0x45
	packet[3] = 20
	packet[8] = 64
	packet[9] = 17
	copy(packet[12:16], []byte{172, 16, 0, 2})
	copy(packet[16:20], []byte{1, 1, 1, 1})
	if _, err := link.packets.WritePacket(packet); err != nil {
		t.Fatal(err)
	}
	buffer := make([]byte, 1500)
	n, err := link.packets.ReadPacket(buffer, true)
	if err != nil || n != 20 || buffer[8] != 62 {
		t.Fatal("H3 CONNECT-IP did not return the datagram:", err)
	}
}

func TestConcurrentMasqueSessionsKeepIdentitySNIAndCancellationIndependent(t *testing.T) {
	a, peer := masqueRequest(t)
	b, _ := masqueRequest(t)
	b.Identity.Masque.PeerPublicKeyDER = a.Identity.Masque.PeerPublicKeyDER
	a.Parameters.SNI = "first.example"
	b.Parameters.SNI = "second.example"
	identities := make(map[string]*ecdsa.PublicKey)
	for _, request := range []wire.ProbeRequest{a, b} {
		der, _ := base64.StdEncoding.DecodeString(request.Identity.Masque.PrivateKeyDER)
		key, _ := x509.ParseECPrivateKey(der)
		identities[request.Parameters.SNI] = &key.PublicKey
	}
	certificate, err := x509.CreateCertificate(rand.Reader, &x509.Certificate{SerialNumber: big.NewInt(6)}, &x509.Certificate{}, &peer.PublicKey, peer)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewUnstartedServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
		w.(http.Flusher).Flush()
		<-r.Context().Done()
	}))
	server.EnableHTTP2 = true
	server.TLS = &tls.Config{Certificates: []tls.Certificate{{Certificate: [][]byte{certificate}, PrivateKey: peer}}, ClientAuth: tls.RequireAnyClientCert,
		VerifyConnection: func(state tls.ConnectionState) error {
			expected := identities[state.ServerName]
			if expected == nil || len(state.PeerCertificates) != 1 {
				return errors.New("session identity missing")
			}
			actual, ok := state.PeerCertificates[0].PublicKey.(*ecdsa.PublicKey)
			if !ok || !actual.Equal(expected) {
				return errors.New("session identity mixed")
			}
			return nil
		}}
	server.StartTLS()
	defer server.Close()
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	ctxA, cancelA := context.WithCancel(ctx)
	defer cancelA()
	ctxB, cancelB := context.WithCancel(ctx)
	defer cancelB()
	type outcome struct {
		index     int
		session   *masqueSession
		connected bool
		err       error
	}
	results := make(chan outcome, 2)
	for index, request := range []wire.ProbeRequest{a, b} {
		request.TimeoutMS = 1000
		request.Targets = []string{server.Listener.Addr().String()}
		lifetime := ctxA
		if index == 1 {
			lifetime = ctxB
		}
		go func() {
			session, connected, err := newMasque(lifetime, request, request.Targets[0])
			results <- outcome{index, session, connected, err}
		}()
	}
	var sessions [2]*masqueSession
	for range 2 {
		result := <-results
		if result.err != nil || !result.connected {
			t.Fatal("independent session authentication failed:", result.err)
		}
		sessions[result.index] = result.session
		defer result.session.Close()
	}
	cancelA()
	select {
	case <-sessions[0].done:
	case <-time.After(time.Second):
		t.Fatal("first session did not close")
	}
	select {
	case <-sessions[1].done:
		t.Fatal("closing first session stopped the second")
	case <-time.After(50 * time.Millisecond):
	}
}
