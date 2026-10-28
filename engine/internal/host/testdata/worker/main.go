package main

import (
	"encoding/binary"
	"encoding/json"
	"os"
	"time"

	"tropa.local/probe/internal/wire"
)

var mode = "fixture-hang"

func main() {
	data, err := wire.ReadFrame(os.Stdin)
	if err != nil {
		os.Exit(20)
	}
	hello, err := wire.DecodeCommand(data)
	if err != nil || hello.Type != "hello" {
		os.Exit(21)
	}
	send(hello, 1, "hello", wire.Hello{BuildID: mode, Transports: []wire.Transport{wire.WireGuard}, Tuning: []string{}})
	data, err = wire.ReadFrame(os.Stdin)
	if err != nil {
		os.Exit(22)
	}
	probe, err := wire.DecodeCommand(data)
	if err != nil || probe.Type != "probe" {
		os.Exit(23)
	}
	send(probe, 1, "accepted", struct{}{})
	switch mode {
	case "fixture-malformed":
		var header [4]byte
		binary.BigEndian.PutUint32(header[:], 262145)
		os.Stdout.Write(header[:])
	case "fixture-exit":
		os.Exit(24)
	case "fixture-panic":
		panic("synthetic worker failure")
	}
	for {
		time.Sleep(time.Hour)
	}
}

func send(command wire.Command, sequence uint64, kind string, payload any) {
	body, _ := json.Marshal(payload)
	event, _ := json.Marshal(wire.Event{Version: wire.Version, SearchID: command.SearchID, JobID: command.JobID, Sequence: sequence, Type: kind, Payload: body})
	if wire.WriteFrame(os.Stdout, event) != nil {
		os.Exit(25)
	}
}
