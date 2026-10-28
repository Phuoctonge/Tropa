package wire

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestCommandRejectsIncompatibleOrAmbiguousEnvelope(t *testing.T) {
	for _, input := range []string{
		`{"version":2,"searchId":"s","jobId":"j","type":"hello","payload":{}}`,
		`{"version":1,"searchId":"","jobId":"j","type":"hello","payload":{}}`,
		`{"version":1,"searchId":"s","jobId":"j","type":"scan-cli","payload":{}}`,
		`{"version":1,"searchId":"s","jobId":"j","type":"hello","payload":{},"token":"secret"}`,
		`{"version":1,"searchId":"s","jobId":"j","type":"hello","payload":{}} {}`,
	} {
		if _, err := DecodeCommand([]byte(input)); err == nil {
			t.Fatal("invalid envelope accepted")
		}
	}
	input := []byte(`{"version":1,"searchId":"s","jobId":"control","type":"hello","payload":{}}`)
	if _, err := DecodeCommand(input); err != nil {
		t.Fatalf("valid hello rejected: %v", err)
	}
}

func TestProbeCommandEnforcesRequiredFieldsAndReservedLength(t *testing.T) {
	valid := `{"version":1,"searchId":"s","jobId":"j","type":"probe","payload":{"transport":"wg","ipv6":false,"targets":["188.114.98.58:2408"],"identity":{"wireGuard":{"privateKey":"synthetic-private","peerPublicKey":"synthetic-peer","ipv4":"172.16.0.2","reserved":[0,0,0]}},"parameters":{},"timeoutMs":2000,"tunnelLimit":4}}`
	if _, err := DecodeCommand([]byte(valid)); err != nil {
		t.Fatal(err)
	}
	for name, input := range map[string]string{
		"missing family":       strings.Replace(valid, `"ipv6":false,`, "", 1),
		"null family":          strings.Replace(valid, `"ipv6":false`, `"ipv6":null`, 1),
		"missing parameters":   strings.Replace(valid, `"parameters":{},`, "", 1),
		"null parameters":      strings.Replace(valid, `"parameters":{}`, `"parameters":null`, 1),
		"missing reserved":     strings.Replace(valid, `,"reserved":[0,0,0]`, "", 1),
		"short reserved":       strings.Replace(valid, `[0,0,0]`, `[1]`, 1),
		"long reserved":        strings.Replace(valid, `[0,0,0]`, `[0,0,0,1]`, 1),
		"empty AWG parameters": strings.Replace(strings.Replace(valid, `"transport":"wg"`, `"transport":"awg"`, 1), `"parameters":{}`, `"parameters":{"awg":{}}`, 1),
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeCommand([]byte(input)); err == nil {
				t.Fatal("schema-incompatible request accepted")
			}
		})
	}
}

func TestCandidatePreservesUnknownMeasurements(t *testing.T) {
	c := Candidate{Endpoint: "162.159.198.2:443", Transport: MasqueH2}
	data, err := json.Marshal(c)
	if err != nil {
		t.Fatal(err)
	}
	for _, key := range []string{`"endpointPingMs":null`, `"tunnelPingMs":null`, `"lossPercent":null`} {
		if !bytes.Contains(data, []byte(key)) {
			t.Fatalf("missing measurement changed to zero: %s", key)
		}
	}
}

func TestProbeRejectsIdentityTransportMismatchWithoutLeakingKey(t *testing.T) {
	r := ProbeRequest{
		Transport: MasqueH2, IPv6: false, Targets: []string{"162.159.198.2:443"},
		Identity:  Identity{WireGuard: &WGIdentity{PrivateKey: "synthetic-private-marker"}},
		TimeoutMS: 2000, TunnelLimit: 4,
	}
	err := r.Validate()
	if err == nil || bytes.Contains([]byte(err.Error()), []byte("synthetic-private-marker")) {
		t.Fatal("identity mismatch accepted or credential leaked")
	}
	data, err := json.Marshal(Candidate{Endpoint: r.Targets[0], Transport: r.Transport})
	if err != nil || bytes.Contains(data, []byte("synthetic-private-marker")) {
		t.Fatal("public candidate contains identity")
	}
}

func TestCommandRejectsInvalidUTF8(t *testing.T) {
	input := []byte(`{"version":1,"searchId":"s","jobId":"j","type":"hello","payload":{}}`)
	input = bytes.Replace(input, []byte(`"s"`), []byte{'"', 0xff, '"'}, 1)
	if _, err := DecodeCommand(input); err == nil {
		t.Fatal("invalid UTF-8 accepted")
	}
}

func TestProbeRejectsMissingIdentityFieldsAndInvalidLimits(t *testing.T) {
	valid := ProbeRequest{
		Transport: WireGuard, Targets: []string{"188.114.98.58:2408"},
		Identity:  Identity{WireGuard: &WGIdentity{PrivateKey: "opaque-private", PeerPublicKey: "opaque-peer", IPv4: "172.16.0.2"}},
		TimeoutMS: 2000, TunnelLimit: 4,
	}
	if err := valid.Validate(); err != nil {
		t.Fatal(err)
	}
	missing := valid
	missing.Identity = Identity{WireGuard: &WGIdentity{}}
	if err := missing.Validate(); err == nil {
		t.Fatal("empty identity accepted")
	}
	wrongFamily := valid
	wrongFamily.IPv6 = true
	if err := wrongFamily.Validate(); err == nil {
		t.Fatal("mixed target family accepted")
	}
	tooMany := valid
	tooMany.TunnelLimit = 5
	if err := tooMany.Validate(); err == nil {
		t.Fatal("tunnel limit exceeded")
	}
}
