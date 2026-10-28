package probe

import (
	"context"
	"encoding/binary"
	"net"
	"strings"
	"testing"
	"time"

	"golang.org/x/net/dns/dnsmessage"
	"tropa.local/probe/internal/wire"
)

func TestGeneratedI1SendsRealDNSSTUNAndSIPPackets(t *testing.T) {
	for _, kind := range []string{"dns", "stun", "sip"} {
		t.Run(kind, func(t *testing.T) {
			chain, err := GenerateI1(kind, "www.google.com")
			if err != nil {
				t.Fatal(err)
			}
			remote, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
			if err != nil {
				t.Fatal(err)
			}
			defer remote.Close()
			remote.SetReadDeadline(time.Now().Add(2 * time.Second))
			request := wgRequest(7)
			request.Parameters.AWG = &wire.AWGParameters{I1: chain}
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			session, err := newWG(ctx, request, remote.LocalAddr().String())
			if err != nil {
				t.Fatal(err)
			}
			defer session.Close()
			done := make(chan struct{})
			go func() { session.Handshake(ctx); close(done) }()
			defer func() { cancel(); <-done }()
			packet := make([]byte, 2048)
			n, _, err := remote.ReadFromUDP(packet)
			if err != nil {
				t.Fatal("no generated preamble received:", err)
			}
			packet = packet[:n]
			switch kind {
			case "dns":
				var query dnsmessage.Message
				if err := query.Unpack(packet); err != nil {
					t.Fatal("invalid DNS preamble:", err)
				}
				if query.Response || !query.RecursionDesired || len(query.Questions) != 1 || query.Questions[0].Name.String() != "www.google.com." || query.Questions[0].Type != dnsmessage.TypeA || len(query.Additionals) != 1 {
					t.Fatal("DNS query semantics changed")
				}
				opt, ok := query.Additionals[0].Body.(*dnsmessage.OPTResource)
				if !ok || len(opt.Options) != 1 || opt.Options[0].Code != 12 || len(opt.Options[0].Data) < 60 {
					t.Fatal("DNS padding is not an EDNS option")
				}
			case "stun":
				if len(packet) < 20 || binary.BigEndian.Uint16(packet[:2]) != 1 || int(binary.BigEndian.Uint16(packet[2:4])) != len(packet)-20 || binary.BigEndian.Uint32(packet[4:8]) != 0x2112a442 || binary.BigEndian.Uint16(packet[20:22]) != 0x8022 {
					t.Fatal("invalid STUN binding packet")
				}
			case "sip":
				value := string(packet)
				if !strings.HasPrefix(value, "OPTIONS sip:www.google.com SIP/2.0\r\n") || !strings.Contains(value, "\r\nCSeq: ") || !strings.HasSuffix(value, "Content-Length: 0\r\n\r\n") {
					t.Fatal("invalid SIP OPTIONS preamble")
				}
			}
		})
	}
}

func TestTuningRejectsUnsupportedOrInjectableProfiles(t *testing.T) {
	for _, value := range []struct{ kind, host string }{
		{"quic", "www.google.com"}, {"unknown", "www.google.com"},
		{"dns", "bad..example"}, {"sip", "example.com\r\ni1=bad"},
		{"dns", strings.Repeat("a", 64) + ".com"}, {"stun", ""},
	} {
		if _, err := GenerateI1(value.kind, value.host); err == nil {
			t.Fatal("unsupported or invalid generation accepted")
		}
	}
}

func TestFiniteTuningProfilesAreIndependentAndKeepEachTransport(t *testing.T) {
	profiles, unavailable := TuningProfiles()
	if len(profiles) > 32 || len(profiles) < 8 {
		t.Fatal("tuning plan is not bounded or omits variants")
	}
	seen := make(map[string]bool)
	kinds := make(map[wire.Transport]bool)
	var awg *wire.AWGParameters
	for _, profile := range profiles {
		if profile.Name == "" || seen[profile.Name] {
			t.Fatal("ambiguous tuning profile")
		}
		seen[profile.Name] = true
		kinds[profile.Transport] = true
		if profile.Transport == wire.AWG {
			if profile.Parameters.AWG == nil {
				t.Fatal("AWG parameters missing")
			}
			awg = profile.Parameters.AWG
		}
		if profile.Transport == wire.MasqueH2 || profile.Transport == wire.MasqueH3 {
			if profile.Parameters.SNI == "" {
				t.Fatal("SNI variant has no actual value")
			}
		}
	}
	for _, kind := range []wire.Transport{wire.WireGuard, wire.AWG, wire.MasqueH3, wire.MasqueH2} {
		if !kinds[kind] {
			t.Fatal("transport omitted from tuning plan")
		}
	}
	if len(unavailable) != 1 || unavailable[0].Name != "awg-quic" || unavailable[0].Reason != "provenance-unresolved" {
		t.Fatal("QUIC limitation is not explicit")
	}
	awg.I1 = "mutation-marker"
	fresh, _ := TuningProfiles()
	for _, profile := range fresh {
		if profile.Parameters.AWG != nil && profile.Parameters.AWG.I1 == "mutation-marker" {
			t.Fatal("tuning plans share mutable parameters")
		}
	}
}
