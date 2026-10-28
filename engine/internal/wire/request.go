package wire

import (
	"bytes"
	"encoding/json"
	"errors"
)

func DecodeProbeRequest(data []byte) (ProbeRequest, error) {
	var request ProbeRequest
	if err := decodeStrict(data, &request); err != nil {
		return ProbeRequest{}, errors.New("invalid probe payload")
	}
	fields, err := requireFields(data, "transport", "ipv6", "targets", "identity", "parameters", "timeoutMs", "tunnelLimit")
	if err != nil {
		return ProbeRequest{}, err
	}
	if err := request.Validate(); err != nil {
		return ProbeRequest{}, err
	}
	family := "masque"
	if request.Transport == WireGuard || request.Transport == AWG {
		family = "wireGuard"
	}
	identity, err := requireFields(fields["identity"], family)
	if err != nil {
		return ProbeRequest{}, err
	}
	if family == "wireGuard" {
		wg, err := requireFields(identity[family], "privateKey", "peerPublicKey", "ipv4", "reserved")
		if err != nil {
			return ProbeRequest{}, err
		}
		var reserved []byte
		// Fixed Go arrays otherwise accept both missing and surplus JSON elements.
		var raw []json.RawMessage
		if json.Unmarshal(wg["reserved"], &raw) != nil || len(raw) != 3 {
			return ProbeRequest{}, errors.New("invalid reserved byte count")
		}
		if json.Unmarshal(wg["reserved"], &reserved) != nil {
			return ProbeRequest{}, errors.New("invalid reserved bytes")
		}
	} else if _, err := requireFields(identity[family], "privateKeyDer", "peerPublicKeyDer", "ipv4"); err != nil {
		return ProbeRequest{}, err
	}
	if request.Transport == AWG {
		parameters, err := requireFields(fields["parameters"], "awg")
		if err != nil {
			return ProbeRequest{}, err
		}
		if _, err := requireFields(parameters["awg"], "jc", "jmin", "jmax", "i1"); err != nil {
			return ProbeRequest{}, err
		}
	}
	return request, nil
}

func requireFields(data []byte, names ...string) (map[string]json.RawMessage, error) {
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil || fields == nil {
		return nil, errors.New("invalid request object")
	}
	for _, name := range names {
		value, ok := fields[name]
		if !ok || bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
			return nil, errors.New("missing required request field")
		}
	}
	return fields, nil
}
