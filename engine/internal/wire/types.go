package wire

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"net/netip"
	"unicode/utf8"
)

const Version = 1

type Transport string

const (
	WireGuard Transport = "wg"
	AWG       Transport = "awg"
	MasqueH3  Transport = "masqueH3"
	MasqueH2  Transport = "masqueH2"
)

type Command struct {
	Version  int             `json:"version"`
	SearchID string          `json:"searchId"`
	JobID    string          `json:"jobId"`
	Type     string          `json:"type"`
	Payload  json.RawMessage `json:"payload"`
}

type Event struct {
	Version  int             `json:"version"`
	SearchID string          `json:"searchId"`
	JobID    string          `json:"jobId"`
	Sequence uint64          `json:"sequence"`
	Type     string          `json:"type"`
	Payload  json.RawMessage `json:"payload"`
}

type Identity struct {
	WireGuard *WGIdentity `json:"wireGuard,omitempty"`
	Masque    *ECIdentity `json:"masque,omitempty"`
}

type WGIdentity struct {
	PrivateKey    string  `json:"privateKey"`
	PeerPublicKey string  `json:"peerPublicKey"`
	IPv4          string  `json:"ipv4"`
	IPv6          string  `json:"ipv6,omitempty"`
	Reserved      [3]byte `json:"reserved"`
}

type ECIdentity struct {
	PrivateKeyDER    string `json:"privateKeyDer"`
	PeerPublicKeyDER string `json:"peerPublicKeyDer"`
	IPv4             string `json:"ipv4"`
	IPv6             string `json:"ipv6,omitempty"`
}

type AWGParameters struct {
	JC   uint16 `json:"jc"`
	JMin uint16 `json:"jmin"`
	JMax uint16 `json:"jmax"`
	I1   string `json:"i1"`
}

type Parameters struct {
	AWG *AWGParameters `json:"awg,omitempty"`
	SNI string         `json:"sni,omitempty"`
}

type ProbeRequest struct {
	Transport   Transport  `json:"transport"`
	IPv6        bool       `json:"ipv6"`
	Targets     []string   `json:"targets"`
	Identity    Identity   `json:"identity"`
	Parameters  Parameters `json:"parameters"`
	TimeoutMS   uint32     `json:"timeoutMs"`
	TunnelLimit uint8      `json:"tunnelLimit"`
}

func (r ProbeRequest) Validate() error {
	if r.TimeoutMS == 0 || r.TimeoutMS > 30000 || r.TunnelLimit == 0 || r.TunnelLimit > 4 || len(r.Targets) == 0 || len(r.Targets) > 256 {
		return errors.New("invalid probe limits")
	}
	for _, target := range r.Targets {
		addr, err := netip.ParseAddrPort(target)
		if err != nil || addr.Port() == 0 || addr.Addr().Is6() != r.IPv6 {
			return errors.New("invalid target family or port")
		}
	}
	switch r.Transport {
	case WireGuard, AWG:
		if r.Identity.WireGuard == nil || r.Identity.Masque != nil || r.Parameters.SNI != "" {
			return errors.New("invalid WireGuard identity or parameters")
		}
		if (r.Transport == AWG) != (r.Parameters.AWG != nil) {
			return errors.New("invalid AWG parameter selection")
		}
		identity := r.Identity.WireGuard
		if identity.PrivateKey == "" || identity.PeerPublicKey == "" || !validAddresses(identity.IPv4, identity.IPv6) {
			return errors.New("incomplete WireGuard identity")
		}
	case MasqueH3, MasqueH2:
		if r.Identity.Masque == nil || r.Identity.WireGuard != nil || r.Parameters.AWG != nil || r.Parameters.SNI == "" {
			return errors.New("invalid MASQUE identity or parameters")
		}
		identity := r.Identity.Masque
		if identity.PrivateKeyDER == "" || identity.PeerPublicKeyDER == "" || !validAddresses(identity.IPv4, identity.IPv6) {
			return errors.New("incomplete MASQUE identity")
		}
	default:
		return errors.New("unsupported transport")
	}
	return nil
}

func validAddresses(v4, v6 string) bool {
	address, err := netip.ParseAddr(v4)
	if err != nil || !address.Is4() {
		return false
	}
	if v6 == "" {
		return true
	}
	address, err = netip.ParseAddr(v6)
	return err == nil && address.Is6()
}

type Stages struct {
	Handshake *bool `json:"handshake"`
	HTTP      *bool `json:"http"`
	HTTPS     *bool `json:"https"`
	UDP       *bool `json:"udp"`
}

type Candidate struct {
	Endpoint       string     `json:"endpoint"`
	Transport      Transport  `json:"transport"`
	Parameters     Parameters `json:"parameters"`
	Stages         Stages     `json:"stages"`
	EndpointPingMS *float64   `json:"endpointPingMs"`
	TunnelPingMS   *float64   `json:"tunnelPingMs"`
	LossPercent    *float64   `json:"lossPercent"`
	JitterMS       *float64   `json:"jitterMs"`
	Samples        uint32     `json:"samples"`
	Method         string     `json:"method"`
	Node           *string    `json:"node"`
	NodeCountry    *string    `json:"nodeCountry"`
	ExitCountry    *string    `json:"exitCountry"`
}

type Hello struct {
	BuildID           string              `json:"buildId"`
	Transports        []Transport         `json:"transports"`
	Tuning            []string            `json:"tuning"`
	ParameterSets     []ParameterSet      `json:"parameterSets,omitempty"`
	UnavailableTuning []UnavailableTuning `json:"unavailableTuning,omitempty"`
}

type ParameterSet struct {
	Name       string     `json:"name"`
	Transport  Transport  `json:"transport"`
	Parameters Parameters `json:"parameters"`
}

type UnavailableTuning struct {
	Name   string `json:"name"`
	Reason string `json:"reason"`
}

type Progress struct {
	Completed uint32 `json:"completed"`
	Total     uint32 `json:"total"`
}

type JobFinished struct {
	Outcome string `json:"outcome"`
}

type Failure struct {
	Stage     string `json:"stage"`
	Reason    string `json:"reason"`
	Retryable bool   `json:"retryable"`
}

func DecodeCommand(data []byte) (Command, error) {
	var command Command
	if err := decodeStrict(data, &command); err != nil {
		return Command{}, errors.New("invalid command JSON")
	}
	if command.Version != Version || command.SearchID == "" || command.JobID == "" || len(command.SearchID) > 128 || len(command.JobID) > 128 {
		return Command{}, errors.New("invalid command version or identifiers")
	}
	switch command.Type {
	case "hello", "cancel", "shutdown":
		var payload struct{}
		if err := decodeStrict(command.Payload, &payload); err != nil || bytes.Equal(bytes.TrimSpace(command.Payload), []byte("null")) {
			return Command{}, errors.New("invalid control payload")
		}
	case "probe":
		if _, err := DecodeProbeRequest(command.Payload); err != nil {
			return Command{}, err
		}
	default:
		return Command{}, errors.New("unsupported command")
	}
	return command, nil
}

func decodeStrict(data []byte, destination any) error {
	if !utf8.Valid(data) {
		return errors.New("invalid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	if err := decoder.Decode(new(any)); !errors.Is(err, io.EOF) {
		return errors.New("unexpected trailing JSON")
	}
	return nil
}
