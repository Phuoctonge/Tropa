package probe

import (
	"encoding/binary"
	"errors"
	"fmt"
	"strings"

	"tropa.local/probe/internal/wire"
)

func GenerateI1(kind, host string) (string, error) {
	if len(host) == 0 || len(host) > 253 {
		return "", errors.New("invalid I1 host")
	}
	var name []byte
	for _, label := range strings.Split(host, ".") {
		if len(label) == 0 || len(label) > 63 || label[0] == '-' || label[len(label)-1] == '-' {
			return "", errors.New("invalid I1 host")
		}
		for _, c := range label {
			if !(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '-') {
				return "", errors.New("invalid I1 host")
			}
		}
		name = append(name, byte(len(label)))
		name = append(name, label...)
	}
	name = append(name, 0)
	literal := func(value []byte) string { return fmt.Sprintf("<b 0x%x>", value) }
	switch kind {
	case "dns":
		packet := []byte{1, 0, 0, 1, 0, 0, 0, 0, 0, 1}
		packet = append(packet, name...)
		packet = append(packet, 0, 1, 0, 1)
		packet = append(packet, 0, 0, 41, 0x10, 0, 0, 0, 0, 0)
		packet = binary.BigEndian.AppendUint16(packet, 100)
		packet = binary.BigEndian.AppendUint16(packet, 12)
		packet = binary.BigEndian.AppendUint16(packet, 96)
		return "<r 2>" + literal(packet) + "<r 96>", nil
	case "stun":
		return literal([]byte{0, 1, 0, 28, 0x21, 0x12, 0xa4, 0x42}) + "<r 12>" + literal([]byte{0x80, 0x22, 0, 24}) + "<rc 24>", nil
	case "sip":
		return literal([]byte("OPTIONS sip:"+host+" SIP/2.0\r\nVia: SIP/2.0/UDP 192.0.2.1:5060;branch=z9hG4bK")) +
			"<rc 10>" + literal([]byte("\r\nFrom: <sip:")) + "<rc 8>" + literal([]byte("@"+host+">;tag=")) + "<rd 6>" +
			literal([]byte("\r\nTo: <sip:"+host+">\r\nCall-ID: ")) + "<rc 16>" + literal([]byte("@"+host+"\r\nCSeq: ")) + "<rd 4>" +
			literal([]byte(" OPTIONS\r\nMax-Forwards: 70\r\nUser-Agent: PJSIP/2.13\r\nContent-Length: 0\r\n\r\n")), nil
	default:
		return "", errors.New("unsupported I1 profile")
	}
}

func TuningProfiles() ([]wire.ParameterSet, []wire.UnavailableTuning) {
	profiles := []wire.ParameterSet{{Name: "wg-default", Transport: wire.WireGuard}}
	addAWG := func(name string, jc, jmin, jmax uint16, i1 string) {
		profiles = append(profiles, wire.ParameterSet{Name: name, Transport: wire.AWG,
			Parameters: wire.Parameters{AWG: &wire.AWGParameters{JC: jc, JMin: jmin, JMax: jmax, I1: i1}}})
	}
	addAWG("awg-default", 6, 10, 50, "<r 2><b 0x858000010001000000000669636c6f756403636f6d0000010001c00c000100010000105a00044d583737>")
	for _, size := range []struct {
		name         string
		jc, min, max uint16
	}{
		{"small", 4, 10, 50}, {"wide", 6, 40, 120},
	} {
		addAWG("awg-junk-"+size.name, size.jc, size.min, size.max, "")
	}
	for _, init := range []struct{ name, kind, host string }{
		{"dns-google", "dns", "www.google.com"}, {"dns-apple", "dns", "www.apple.com"},
		{"sip-google", "sip", "www.google.com"}, {"sip-apple", "sip", "www.apple.com"}, {"stun", "stun", "www.google.com"},
	} {
		chain, err := GenerateI1(init.kind, init.host)
		if err != nil {
			panic("invalid built-in I1 profile")
		}
		for _, size := range []struct {
			name         string
			jc, min, max uint16
		}{
			{"i1", 0, 0, 0}, {"small", 4, 10, 50}, {"wide", 6, 40, 120},
		} {
			addAWG("awg-"+init.name+"-"+size.name, size.jc, size.min, size.max, chain)
		}
	}
	for _, kind := range []wire.Transport{wire.MasqueH3, wire.MasqueH2} {
		for _, sni := range []struct{ name, host string }{
			{"default", "consumer-masque.cloudflareclient.com"}, {"apple", "www.apple.com"},
			{"google", "www.google.com"}, {"microsoft", "www.microsoft.com"}, {"jsdelivr", "cdn.jsdelivr.net"},
		} {
			profiles = append(profiles, wire.ParameterSet{Name: string(kind) + "-" + sni.name, Transport: kind, Parameters: wire.Parameters{SNI: sni.host}})
		}
	}
	return profiles, []wire.UnavailableTuning{{Name: "awg-quic", Reason: "provenance-unresolved"}}
}
