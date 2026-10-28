package main

import (
	"context"
	"fmt"
	"io"
	"log"
	"os"

	"tropa.local/probe/internal/host"
	"tropa.local/probe/internal/probe"
)

var buildID = "development"

func main() {
	log.SetOutput(io.Discard)
	profiles, unavailable := probe.TuningProfiles()
	if err := host.Serve(context.Background(), os.Stdin, os.Stdout, host.Options{BuildID: buildID, Probe: probe.Probe, ParameterSets: profiles, UnavailableTuning: unavailable}); err != nil {
		fmt.Fprintln(os.Stderr, "tropa_probe_failed")
		os.Exit(1)
	}
}
