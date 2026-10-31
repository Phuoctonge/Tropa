package probe

import (
	"bytes"
	"context"
	"crypto/ecdh"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"time"

	"tropa.local/probe/internal/wire"
)

func TestRegistrationPipeForwardsEnrollmentWithoutReplayingPost(t *testing.T) {
	var requests []string
	api := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		requests = append(requests, r.Method+" "+r.URL.Path+" "+string(body))
		if r.Method == "PATCH" && r.Header.Get("Authorization") != "Bearer synthetic-token" {
			t.Error("activation lost its bearer token")
		}
		if r.Header.Get("CF-Client-Version") != "a-6.11-2223" {
			t.Error("registration lost its API version")
		}
		w.Header().Set("Content-Type", "application/json")
		if r.Method == "POST" {
			w.WriteHeader(429)
		}
		io.WriteString(w, `{"id":"synthetic-device"}`)
	}))
	defer api.Close()
	client := api.Client()
	transport := client.Transport.(*http.Transport)
	transport.TLSClientConfig.ServerName = "example.com"
	transport.DialContext = func(ctx context.Context, _, _ string) (net.Conn, error) {
		return (&net.Dialer{}).DialContext(ctx, "tcp", api.Listener.Addr().String())
	}
	var input, output bytes.Buffer
	for _, request := range []registrationRequest{
		{Method: "POST", Path: "/v0a4005/reg", Body: json.RawMessage(`{"key":"public-only"}`)},
		{Method: "PATCH", Path: "/v0a4005/reg/synthetic-device", Token: "synthetic-token", Body: json.RawMessage(`{"warp_enabled":true}`)},
	} {
		data, _ := json.Marshal(request)
		if err := wire.WriteFrame(&input, data); err != nil {
			t.Fatal(err)
		}
	}
	if err := registrationPipe(context.Background(), &input, &output, client); err != nil {
		t.Fatal(err)
	}
	for _, status := range []int{429, 200} {
		data, err := wire.ReadFrame(&output)
		if err != nil {
			t.Fatal(err)
		}
		var response registrationResponse
		if err := json.Unmarshal(data, &response); err != nil || response.Status != status {
			t.Fatalf("wrong response: %s", data)
		}
	}
	if len(requests) != 2 || !strings.HasPrefix(requests[0], "POST /v0a4005/reg ") || !strings.HasPrefix(requests[1], "PATCH /v0a4005/reg/synthetic-device ") {
		t.Fatalf("registration was replayed or misrouted: %v", requests)
	}
	input.Reset()
	data, _ := json.Marshal(registrationRequest{Method: "POST", Path: "/unrelated", Body: json.RawMessage(`{}`)})
	wire.WriteFrame(&input, data)
	if registrationPipe(context.Background(), &input, &output, client) == nil || len(requests) != 2 {
		t.Fatal("worker forwarded a non-registration request")
	}
}

func TestRegistrationDiscoveryCancellation(t *testing.T) {
	profiles, _ := TuningProfiles()
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Millisecond)
	defer cancel()
	started := time.Now()
	session := discoverRegistrationProfile(ctx, context.Background(), profiles[0], []string{"127.0.0.1:9"}, "127.0.0.1:443")
	if session != nil || time.Since(started) > time.Second {
		t.Fatal("cancelled bootstrap discovery retained a tunnel or ignored cancellation")
	}
}

func TestLiveRegistrationTunnel(t *testing.T) {
	if os.Getenv("TROPA_LIVE_REGISTRATION") != "1" {
		t.Skip("creates a real Cloudflare test device through the bootstrap tunnel")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 110*time.Second)
	defer cancel()
	session, client, err := discoverRegistration(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer session.Close()
	defer client.CloseIdleConnections()
	key, err := ecdh.X25519().GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	exchange := func(request registrationRequest) registrationResponse {
		t.Helper()
		var input, output bytes.Buffer
		data, _ := json.Marshal(request)
		if err := wire.WriteFrame(&input, data); err != nil {
			t.Fatal(err)
		}
		if err := registrationPipe(ctx, &input, &output, client); err != nil {
			t.Fatal(err)
		}
		data, err := wire.ReadFrame(&output)
		if err != nil {
			t.Fatal(err)
		}
		var response registrationResponse
		if err := json.Unmarshal(data, &response); err != nil {
			t.Fatal("invalid registration response")
		}
		if response.Status < 200 || response.Status >= 300 {
			t.Fatalf("API rejected enrollment: HTTP %d", response.Status)
		}
		return response
	}
	body, _ := json.Marshal(map[string]string{"key": base64.StdEncoding.EncodeToString(key.PublicKey().Bytes())})
	created := exchange(registrationRequest{Method: "POST", Path: "/v0a4005/reg", Body: body})
	var identity struct {
		ID    string `json:"id"`
		Token string `json:"token"`
	}
	if json.Unmarshal(created.Body, &identity) != nil || identity.ID == "" || identity.Token == "" {
		t.Fatal("API did not return a device identity")
	}
	exchange(registrationRequest{Method: "PATCH", Path: "/v0a4005/reg/" + identity.ID, Token: identity.Token, Body: json.RawMessage(`{"warp_enabled":true}`)})
	t.Log("fresh device created and activated through bootstrap tunnel; direct registration path was not used")
}
