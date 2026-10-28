package wire

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"os"
	"testing"
)

func TestSharedProtocolFrames(t *testing.T) {
	data, err := os.ReadFile("../../../protocol/fixtures/search-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Cases []struct {
			Name, Kind, JSON, FrameHex string
			Valid                      bool
		}
	}
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	for _, entry := range fixture.Cases {
		t.Run(entry.Name, func(t *testing.T) {
			frame, err := hex.DecodeString(entry.FrameHex)
			if err != nil {
				t.Fatal(err)
			}
			payload, err := ReadFrame(bytes.NewReader(frame))
			if err != nil {
				t.Fatal(err)
			}
			if string(payload) != entry.JSON {
				t.Fatal("framing payload differs from the cross-language fixture")
			}
			var encoded bytes.Buffer
			if err := WriteFrame(&encoded, payload); err != nil || !bytes.Equal(encoded.Bytes(), frame) {
				t.Fatal("framing differs from independently encoded fixture")
			}
			if entry.Kind == "command" {
				_, err := DecodeCommand(payload)
				if (err == nil) != entry.Valid {
					t.Fatal("command version validation differs from fixture")
				}
			}
			if entry.Name == "candidate-null-metrics" {
				var event Event
				if err := json.Unmarshal(payload, &event); err != nil {
					t.Fatal(err)
				}
				var candidate Candidate
				if err := json.Unmarshal(event.Payload, &candidate); err != nil {
					t.Fatal(err)
				}
				if candidate.TunnelPingMS != nil || candidate.LossPercent != nil || candidate.Stages.UDP != nil {
					t.Fatal("unknown metrics became measured values")
				}
			}
		})
	}
}
