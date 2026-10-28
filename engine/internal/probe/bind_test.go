package probe

import (
	"bytes"
	"net"
	"testing"
	"time"
)

func TestReservedBytesDoNotRewriteAWGPreamble(t *testing.T) {
	remote, err := net.ListenUDP("udp4", &net.UDPAddr{IP: net.IPv4(127, 0, 0, 1)})
	if err != nil {
		t.Fatal(err)
	}
	defer remote.Close()
	remote.SetDeadline(time.Now().Add(time.Second))
	bind := &udpBind{reserved: [3]byte{1, 2, 3}}
	if _, _, err := bind.Open(0); err != nil {
		t.Fatal(err)
	}
	defer bind.Close()
	destination, _ := bind.ParseEndpoint(remote.LocalAddr().String())
	// I1 may deliberately resemble a WireGuard header; it precedes the real initiation in one batch.
	i1 := make([]byte, 148)
	i1[0] = 1
	initiation := make([]byte, 148)
	initiation[0] = 1
	expected := bytes.Clone(i1)
	if err := bind.Send([][]byte{i1, initiation}, destination); err != nil {
		t.Fatal(err)
	}
	buffer := make([]byte, 200)
	n, _, err := remote.ReadFromUDPAddrPort(buffer)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(buffer[:n], expected) {
		t.Fatal("AWG I1 bytes were changed by WARP client-ID injection")
	}
	if _, _, err := remote.ReadFromUDPAddrPort(buffer); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(buffer[1:4], []byte{1, 2, 3}) {
		t.Fatal("real initiation lost its WARP client ID")
	}
}
