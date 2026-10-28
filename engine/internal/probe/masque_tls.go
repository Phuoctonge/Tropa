package probe

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"encoding/base64"
	"errors"
	"math/big"
	"strings"
	"time"

	"github.com/Diniboy1123/usque/api"
	"tropa.local/probe/internal/wire"
)

func masqueTLS(request wire.ProbeRequest) (*tls.Config, error) {
	if err := request.Validate(); err != nil {
		return nil, err
	}
	if request.Transport != wire.MasqueH2 && request.Transport != wire.MasqueH3 {
		return nil, errors.New("invalid MASQUE transport")
	}
	if len(request.Parameters.SNI) > 253 || strings.ContainsAny(request.Parameters.SNI, " \t\r\n\x00") {
		return nil, errors.New("invalid MASQUE SNI")
	}
	identity := request.Identity.Masque
	privateDER, err := base64.StdEncoding.Strict().DecodeString(identity.PrivateKeyDER)
	if err != nil {
		return nil, errors.New("invalid MASQUE private key encoding")
	}
	private, err := x509.ParseECPrivateKey(privateDER)
	if err != nil || private.Curve != elliptic.P256() {
		return nil, errors.New("invalid MASQUE private key")
	}
	peerDER, err := base64.StdEncoding.Strict().DecodeString(identity.PeerPublicKeyDER)
	if err != nil {
		return nil, errors.New("invalid MASQUE peer key encoding")
	}
	parsed, err := x509.ParsePKIXPublicKey(peerDER)
	if err != nil {
		return nil, errors.New("invalid MASQUE peer key")
	}
	peer, ok := parsed.(*ecdsa.PublicKey)
	if !ok {
		return nil, errors.New("invalid MASQUE peer key type")
	}
	now := time.Now()
	certificate, err := x509.CreateCertificate(rand.Reader, &x509.Certificate{SerialNumber: big.NewInt(1), NotBefore: now.Add(-time.Minute), NotAfter: now.Add(24 * time.Hour)}, &x509.Certificate{}, &private.PublicKey, private)
	if err != nil {
		return nil, errors.New("MASQUE client certificate creation failed")
	}
	configuration, err := api.PrepareTlsConfig(private, peer, [][]byte{certificate}, request.Parameters.SNI, false)
	if err != nil {
		return nil, errors.New("MASQUE TLS configuration failed")
	}
	verify := configuration.VerifyPeerCertificate
	// The enrolled public key authenticates this endpoint; an absent certificate must not bypass its pin.
	configuration.VerifyPeerCertificate = func(certificates [][]byte, chains [][]*x509.Certificate) error {
		if len(certificates) == 0 {
			return errors.New("missing MASQUE peer certificate")
		}
		return verify(certificates, chains)
	}
	return configuration, nil
}
