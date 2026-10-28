package probe

import (
	"encoding/base64"
	"encoding/hex"
	"errors"
	"fmt"
	"net/netip"
	"strings"

	"tropa.local/probe/internal/wire"
)

func wgConfiguration(request wire.ProbeRequest, target string) (string, error) {
	if err := request.Validate(); err != nil {
		return "", err
	}
	if request.Transport != wire.WireGuard && request.Transport != wire.AWG {
		return "", errors.New("invalid WG transport")
	}
	endpoint, err := netip.ParseAddrPort(target)
	if err != nil || endpoint.Port() == 0 || endpoint.Addr().Is6() != request.IPv6 {
		return "", errors.New("invalid WG endpoint")
	}
	private, err := wgKey(request.Identity.WireGuard.PrivateKey)
	if err != nil {
		return "", err
	}
	peer, err := wgKey(request.Identity.WireGuard.PeerPublicKey)
	if err != nil {
		return "", err
	}
	var configuration strings.Builder
	fmt.Fprintf(&configuration, "private_key=%s\n", private)
	if awg := request.Parameters.AWG; awg != nil {
		if (awg.JC > 0 && (awg.JMin == 0 || awg.JMax < awg.JMin)) || strings.ContainsAny(awg.I1, "\r\n\x00") {
			return "", errors.New("invalid AWG parameters")
		}
		// The library defaults to no junk but rejects an explicit jc=0 in UAPI.
		if awg.JC > 0 {
			fmt.Fprintf(&configuration, "jc=%d\njmin=%d\njmax=%d\n", awg.JC, awg.JMin, awg.JMax)
		}
		if awg.I1 != "" {
			fmt.Fprintf(&configuration, "i1=%s\n", awg.I1)
		}
	}
	fmt.Fprintf(&configuration, "replace_peers=true\npublic_key=%s\nendpoint=%s\nallowed_ip=0.0.0.0/0\nallowed_ip=::/0\npersistent_keepalive_interval=0\n", peer, endpoint)
	return configuration.String(), nil
}

func wgKey(encoded string) (string, error) {
	key, err := base64.StdEncoding.Strict().DecodeString(encoded)
	if err != nil || len(key) != 32 {
		return "", errors.New("invalid WG key encoding or length")
	}
	return hex.EncodeToString(key), nil
}
