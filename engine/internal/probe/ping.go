package probe

import (
	"bytes"
	"context"
	"crypto/rand"
	"math"
	"net/netip"
	"sync"
	"time"

	"github.com/amnezia-vpn/amneziawg-go/tun/netstack"
	"golang.org/x/net/icmp"
	"golang.org/x/net/ipv4"
	"tropa.local/probe/internal/wire"
)

func measurePing(parent context.Context, network *netstack.Net, destination netip.Addr, timeout time.Duration) wire.Candidate {
	result := wire.Candidate{}
	if !destination.Is4() || parent.Err() != nil {
		return result
	}
	ctx, cancel := context.WithTimeout(parent, timeout)
	defer cancel()
	connection, err := network.DialPingAddr(netip.Addr{}, destination)
	if err != nil {
		return result
	}
	var closed sync.Once
	closeConnection := func() { closed.Do(func() { connection.Close() }) }
	stop := context.AfterFunc(ctx, closeConnection)
	defer stop()
	defer closeConnection()
	payload := make([]byte, 16)
	rand.Read(payload)
	buffer := make([]byte, 1500)
	var rtts []float64
	for sequence := range 5 {
		if ctx.Err() != nil {
			break
		}
		message := icmp.Message{Type: ipv4.ICMPTypeEcho, Body: &icmp.Echo{ID: 0xbeef, Seq: sequence, Data: payload}}
		packet, err := message.Marshal(nil)
		if err != nil {
			break
		}
		result.Samples++
		start := time.Now()
		if _, err := connection.Write(packet); err == nil {
			deadline := start.Add(max(timeout/5, time.Millisecond))
			if limit, ok := ctx.Deadline(); ok && limit.Before(deadline) {
				deadline = limit
			}
			connection.SetReadDeadline(deadline)
			for ctx.Err() == nil {
				n, err := connection.Read(buffer)
				if err != nil {
					break
				}
				reply, err := icmp.ParseMessage(1, buffer[:n])
				if err != nil || reply.Type != ipv4.ICMPTypeEchoReply {
					continue
				}
				echo, ok := reply.Body.(*icmp.Echo)
				if !ok || echo.Seq != sequence || !bytes.Equal(echo.Data, payload) {
					continue
				}
				rtts = append(rtts, float64(time.Since(start))/float64(time.Millisecond))
				break
			}
		}
		if sequence < 4 {
			timer := time.NewTimer(time.Until(start.Add(200 * time.Millisecond)))
			select {
			case <-ctx.Done():
			case <-timer.C:
			}
			timer.Stop()
		}
	}
	if result.Samples == 0 {
		return result
	}
	result.Method = "icmp-v4-sequential-v1:" + destination.String()
	loss := 100 * (1 - float64(len(rtts))/float64(result.Samples))
	result.LossPercent = &loss
	if len(rtts) > 0 {
		var sum float64
		for _, value := range rtts {
			sum += value
		}
		mean := sum / float64(len(rtts))
		result.TunnelPingMS = &mean
	}
	if len(rtts) > 1 {
		var difference float64
		for i := 1; i < len(rtts); i++ {
			difference += math.Abs(rtts[i] - rtts[i-1])
		}
		jitter := difference / float64(len(rtts)-1)
		result.JitterMS = &jitter
	}
	return result
}
