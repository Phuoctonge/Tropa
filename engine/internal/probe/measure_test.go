package probe

import (
	"context"
	"net"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

func TestTraceRequiresWARPAndRejectsRedirects(t *testing.T) {
	for _, body := range []string{"warp=off\nloc=DE\n", "warp=onward\n", "warp=on\n" + strings.Repeat("x", 17000)} {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.Write([]byte(body)) }))
		ok, _ := trace(context.Background(), server.Client(), server.URL)
		server.Close()
		if ok {
			t.Fatal("unverified trace was accepted")
		}
	}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/" {
			http.Redirect(w, r, "/success", http.StatusFound)
			return
		}
		w.Write([]byte("warp=on\nloc=DE\ncolo=FRA\n"))
	}))
	defer server.Close()
	client := probeHTTPClient((&net.Dialer{}).DialContext)
	if ok, _ := trace(context.Background(), client, server.URL); ok {
		t.Fatal("redirect crossed measurement boundary")
	}
}

func TestDataStagesKeepHTTPAndHTTPSSeparate(t *testing.T) {
	httpServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.Write([]byte("warp=on\nloc=NL\ncolo=AMS\n")) }))
	defer httpServer.Close()
	tlsServer := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.Write([]byte("warp=off\n")) }))
	defer tlsServer.Close()
	result := measureData(context.Background(), tlsServer.Client(), traceTargets{HTTP: httpServer.URL, HTTPS: tlsServer.URL}, time.Second)
	if result.Stages.HTTP == nil || !*result.Stages.HTTP || result.Stages.HTTPS == nil || *result.Stages.HTTPS {
		t.Fatal("HTTP result was used as HTTPS success")
	}
	if result.Stages.UDP != nil || result.NodeCountry != nil || result.ExitCountry != nil {
		t.Fatal("unverified UDP or geography was invented")
	}
}

func TestTraceCancellationStopsRequest(t *testing.T) {
	entered := make(chan struct{})
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { close(entered); <-r.Context().Done() }))
	defer server.Close()
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan struct{})
	go func() { trace(ctx, server.Client(), server.URL); close(done) }()
	<-entered
	cancel()
	select {
	case <-done:
	case <-time.After(time.Second):
		t.Fatal("cancelled HTTP request did not exit")
	}
}

func TestVerifiedHTTPSReportsExitCountryWithoutInventingNodeCountry(t *testing.T) {
	server := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.Write([]byte("warp=plus\nloc=NL\ncolo=AMS\n")) }))
	defer server.Close()
	result := measureData(context.Background(), server.Client(), traceTargets{HTTP: server.URL, HTTPS: server.URL}, time.Second)
	if result.Stages.HTTPS == nil || !*result.Stages.HTTPS || result.ExitCountry == nil || *result.ExitCountry != "NL" || result.Node == nil || *result.Node != "AMS" {
		t.Fatal("verified HTTPS trace metadata was lost")
	}
	if result.NodeCountry != nil || result.Stages.UDP != nil {
		t.Fatal("unmeasured node geography or UDP result was invented")
	}
}
