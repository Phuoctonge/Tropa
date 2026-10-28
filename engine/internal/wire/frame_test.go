package wire

import (
	"bytes"
	"errors"
	"io"
	"testing"
)

func TestFrameRejectsOversizedHeaderBeforePayload(t *testing.T) {
	_, err := ReadFrame(bytes.NewReader([]byte{0, 4, 0, 1}))
	if !errors.Is(err, ErrFrameSize) {
		t.Fatalf("expected size rejection, got %v", err)
	}
}

func TestFrameRoundTripWithPartialIO(t *testing.T) {
	w := &partialWriter{}
	want := []byte(`{"version":1,"type":"hello"}`)
	if err := WriteFrame(w, want); err != nil {
		t.Fatal(err)
	}
	r := &partialReader{bytes.NewReader(w.Bytes())}
	got, err := ReadFrame(r)
	if err != nil || !bytes.Equal(got, want) {
		t.Fatalf("round trip: %q, %v", got, err)
	}
	if _, err := ReadFrame(r); !errors.Is(err, io.EOF) {
		t.Fatalf("expected clean EOF, got %v", err)
	}
}

func TestFrameRejectsTruncationAndStalledWriter(t *testing.T) {
	for _, input := range [][]byte{{0, 0}, {0, 0, 0, 2, 'x'}} {
		if _, err := ReadFrame(bytes.NewReader(input)); !errors.Is(err, io.ErrUnexpectedEOF) {
			t.Fatalf("expected truncated frame, got %v", err)
		}
	}
	if err := WriteFrame(stalledWriter{}, []byte("x")); !errors.Is(err, io.ErrShortWrite) {
		t.Fatalf("expected stalled writer rejection, got %v", err)
	}
	if _, err := ReadFrame(bytes.NewReader([]byte{0, 0, 0, 0})); !errors.Is(err, ErrFrameSize) {
		t.Fatalf("expected empty frame rejection, got %v", err)
	}
}

type partialWriter struct{ bytes.Buffer }

func (w *partialWriter) Write(p []byte) (int, error) {
	return w.Buffer.Write(p[:min(2, len(p))])
}

type partialReader struct{ *bytes.Reader }

func (r *partialReader) Read(p []byte) (int, error) {
	return r.Reader.Read(p[:min(1, len(p))])
}

type stalledWriter struct{}

func (stalledWriter) Write([]byte) (int, error) { return 0, nil }
