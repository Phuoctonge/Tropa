package probe

import (
	"bytes"
	"context"
	"crypto/ecdh"
	"encoding/base64"
	"encoding/hex"
	"io"
	"net"
	"net/http"
	"net/netip"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/conn"

	"tropa.local/probe/internal/wire"
)

func wgRequest(seed byte) wire.ProbeRequest {
	return wire.ProbeRequest{
		Transport: wire.AWG, Targets: []string{"127.0.0.1:2408"}, TimeoutMS: 100, TunnelLimit: 1,
		Identity: wire.Identity{WireGuard: &wire.WGIdentity{
			PrivateKey:    base64.StdEncoding.EncodeToString(bytes.Repeat([]byte{seed}, 32)),
			PeerPublicKey: base64.StdEncoding.EncodeToString(bytes.Repeat([]byte{9}, 32)),
			IPv4:          "172.16.0.2", Reserved: [3]byte{seed, 2, 3},
		}},
		Parameters: wire.Parameters{AWG: &wire.AWGParameters{JC: 3, JMin: 40, JMax: 80, I1: "<b 0x0102>"}},
	}
}

func TestSessionIsolation(t *testing.T) {
	requests := []wire.ProbeRequest{wgRequest(1), wgRequest(2)}
	requests[1].Targets[0] = "127.0.0.1:500"
	requests[1].Parameters.AWG.I1 = "<b 0x0304>"
	var group sync.WaitGroup
	for _, request := range requests {
		group.Go(func() {
			configuration, err := wgConfiguration(request, request.Targets[0])
			if err != nil {
				t.Error("valid independent session rejected")
				return
			}
			if !strings.Contains(configuration, "endpoint="+request.Targets[0]+"\n") ||
				!strings.Contains(configuration, "i1="+request.Parameters.AWG.I1+"\n") ||
				!strings.Contains(configuration, "private_key="+hex.EncodeToString(bytes.Repeat([]byte{request.Identity.WireGuard.Reserved[0]}, 32))+"\n") {
				t.Error("session parameters were mixed")
			}
		})
	}
	group.Wait()
}

func TestWGCancelReleasesSocket(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	session, err := newWG(ctx, wgRequest(1), "127.0.0.1:2408")
	if err != nil {
		t.Fatal("userspace session did not start:", err)
	}
	address := session.bind.localAddr()
	cancel()
	select {
	case <-session.done:
	case <-time.After(3 * time.Second):
		t.Fatal("cancellation did not close device")
	}
	connection, err := net.ListenPacket("udp", address)
	if err != nil {
		t.Fatal("owned UDP socket was not released")
	}
	connection.Close()
	session.Close()
}

func TestWARPReservedBytesRoundTrip(t *testing.T) {
	remote, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	defer remote.Close()
	remote.SetDeadline(time.Now().Add(time.Second))
	bind := &udpBind{reserved: [3]byte{12, 34, 56}}
	receivers, _, err := bind.Open(0)
	if err != nil {
		t.Fatal(err)
	}
	defer bind.Close()
	bind.connection.SetDeadline(time.Now().Add(time.Second))
	destination, err := bind.ParseEndpoint(remote.LocalAddr().String())
	if err != nil {
		t.Fatal(err)
	}
	packet := make([]byte, 32)
	packet[0] = 4
	if err := bind.Send([][]byte{packet}, destination); err != nil {
		t.Fatal(err)
	}
	buffer := make([]byte, 64)
	n, sender, err := remote.ReadFromUDPAddrPort(buffer)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(buffer[1:4], []byte{12, 34, 56}) {
		t.Fatal("registration client ID was not sent")
	}
	if _, err := remote.WriteToUDPAddrPort(buffer[:n], sender); err != nil {
		t.Fatal(err)
	}
	sizes := make([]int, 1)
	endpoints := make([]conn.Endpoint, 1)
	if _, err := receivers[0]([][]byte{buffer}, sizes, endpoints); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(buffer[1:4], []byte{0, 0, 0}) {
		t.Fatal("reserved bytes were not normalized for WG decoding")
	}
}

func TestAWGWithoutJunkStartsWithI1(t *testing.T) {
	request := wgRequest(1)
	request.Parameters.AWG.JC = 0
	request.Parameters.AWG.JMin = 0
	request.Parameters.AWG.JMax = 0
	session, err := newWG(context.Background(), request, request.Targets[0])
	if err != nil {
		t.Fatal("I1-only AWG profile rejected:", err)
	}
	defer session.Close()
}

func TestWGRejectsInvalidKeyAndUAPIInjection(t *testing.T) {
	for _, mutate := range []func(*wire.ProbeRequest){
		func(r *wire.ProbeRequest) { r.Identity.WireGuard.PrivateKey = "invalid-secret" },
		func(r *wire.ProbeRequest) {
			r.Identity.WireGuard.PeerPublicKey = base64.StdEncoding.EncodeToString([]byte{1})
		},
		func(r *wire.ProbeRequest) { r.Parameters.AWG.I1 = "<b 0x01>\nprivate_key=invalid-secret" },
		func(r *wire.ProbeRequest) { r.Parameters.AWG.JMin = 90 },
	} {
		r := wgRequest(1)
		mutate(&r)
		configuration, err := wgConfiguration(r, r.Targets[0])
		if err == nil || configuration != "" || strings.Contains(err.Error(), "invalid-secret") {
			t.Error("invalid configuration accepted or secret included in error")
		}
	}
}

func TestUserspacePeersHandshakeAndPassHTTP(t *testing.T) {
	for _, transport := range []wire.Transport{wire.WireGuard, wire.AWG} {
		t.Run(string(transport), func(t *testing.T) {
			a, b := wgRequest(1), wgRequest(2)
			a.Transport, b.Transport = transport, transport
			if transport == wire.WireGuard {
				a.Parameters.AWG, b.Parameters.AWG = nil, nil
			}
			privateA, _ := ecdh.X25519().NewPrivateKey(bytes.Repeat([]byte{1}, 32))
			privateB, _ := ecdh.X25519().NewPrivateKey(bytes.Repeat([]byte{2}, 32))
			a.Identity.WireGuard.PeerPublicKey = base64.StdEncoding.EncodeToString(privateB.PublicKey().Bytes())
			b.Identity.WireGuard.PeerPublicKey = base64.StdEncoding.EncodeToString(privateA.PublicKey().Bytes())
			b.Identity.WireGuard.IPv4 = "172.16.0.3"
			ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
			defer cancel()
			left, err := newWG(ctx, a, a.Targets[0])
			if err != nil {
				t.Fatal(err)
			}
			defer left.Close()
			_, portA, _ := net.SplitHostPort(left.bind.localAddr())
			right, err := newWG(ctx, b, "127.0.0.1:"+portA)
			if err != nil {
				t.Fatal(err)
			}
			defer right.Close()
			_, portB, _ := net.SplitHostPort(right.bind.localAddr())
			configuration, err := wgConfiguration(a, "127.0.0.1:"+portB)
			if err != nil {
				t.Fatal(err)
			}
			if err := left.device.IpcSet(configuration); err != nil {
				t.Fatal("peer endpoint update failed")
			}
			if !left.Handshake(ctx) {
				t.Fatal("local encrypted handshake did not complete")
			}
			ping := measurePing(ctx, left.network, netip.MustParseAddr("172.16.0.3"), 2*time.Second)
			if ping.Samples != 5 || ping.LossPercent == nil || *ping.LossPercent != 0 || ping.TunnelPingMS == nil || ping.JitterMS == nil {
				t.Fatal("userspace ICMP measurements did not count actual replies")
			}
			listener, err := right.network.ListenTCP(&net.TCPAddr{IP: net.ParseIP("172.16.0.3"), Port: 8080})
			if err != nil {
				t.Fatal(err)
			}
			server := &http.Server{Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.Write([]byte("userspace-peer")) })}
			defer server.Close()
			go server.Serve(listener)
			client := probeHTTPClient(left.DialContext)
			defer client.CloseIdleConnections()
			request, _ := http.NewRequestWithContext(ctx, http.MethodGet, "http://172.16.0.3:8080", nil)
			response, err := client.Do(request)
			if err != nil {
				t.Fatal("userspace HTTP failed:", err)
			}
			defer response.Body.Close()
			body, err := io.ReadAll(response.Body)
			if err != nil || string(body) != "userspace-peer" {
				t.Fatal("encrypted stream did not deliver the expected response")
			}
		})
	}
}
