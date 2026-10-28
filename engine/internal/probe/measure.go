package probe

import (
	"context"
	"io"
	"net"
	"net/http"
	"strings"
	"time"

	"tropa.local/probe/internal/wire"
)

type traceTargets struct{ HTTP, HTTPS string }

type traceMetadata struct{ country, colo string }

func probeHTTPClient(dial func(context.Context, string, string) (net.Conn, error)) *http.Client {
	return &http.Client{
		Transport:     &http.Transport{DialContext: dial, DisableKeepAlives: true, ForceAttemptHTTP2: false},
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
	}
}

func measureData(ctx context.Context, client *http.Client, targets traceTargets, timeout time.Duration) wire.Candidate {
	result := wire.Candidate{}
	if ctx.Err() != nil {
		return result
	}
	stage, cancel := context.WithTimeout(ctx, timeout)
	httpOK, _ := trace(stage, client, targets.HTTP)
	cancel()
	result.Stages.HTTP = &httpOK
	if ctx.Err() != nil {
		return result
	}
	stage, cancel = context.WithTimeout(ctx, timeout)
	httpsOK, metadata := trace(stage, client, targets.HTTPS)
	cancel()
	result.Stages.HTTPS = &httpsOK
	if httpsOK {
		if metadata.country != "" {
			result.ExitCountry = &metadata.country
		}
		if metadata.colo != "" {
			result.Node = &metadata.colo
		}
	}
	return result
}

func trace(ctx context.Context, client *http.Client, target string) (bool, traceMetadata) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, target, nil)
	if err != nil {
		return false, traceMetadata{}
	}
	response, err := client.Do(req)
	if err != nil {
		return false, traceMetadata{}
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return false, traceMetadata{}
	}
	const maxTraceBytes = 16384
	body, err := io.ReadAll(io.LimitReader(response.Body, maxTraceBytes+1))
	if err != nil || len(body) > maxTraceBytes {
		return false, traceMetadata{}
	}
	warp := ""
	metadata := traceMetadata{}
	for line := range strings.SplitSeq(string(body), "\n") {
		key, value, ok := strings.Cut(strings.TrimSpace(line), "=")
		if !ok {
			continue
		}
		switch key {
		case "warp":
			warp = value
		case "loc":
			if len(value) == 2 && asciiUpper(value) {
				metadata.country = value
			}
		case "colo":
			if len(value) == 3 && asciiUpper(value) {
				metadata.colo = value
			}
		}
	}
	return warp == "on" || warp == "plus", metadata
}

func asciiUpper(value string) bool {
	for _, letter := range value {
		if letter < 'A' || letter > 'Z' {
			return false
		}
	}
	return true
}
