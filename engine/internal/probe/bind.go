package probe

import (
	"net"
	"net/netip"
	"sync"

	"github.com/amnezia-vpn/amneziawg-go/conn"
)

type udpBind struct {
	mu         sync.Mutex
	connection *net.UDPConn
	reserved   [3]byte
}

type endpoint struct{ address netip.AddrPort }

func (b *udpBind) Open(port uint16) ([]conn.ReceiveFunc, uint16, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.connection != nil {
		return nil, 0, conn.ErrBindAlreadyOpen
	}
	c, err := net.ListenUDP("udp", &net.UDPAddr{Port: int(port)})
	if err != nil {
		return nil, 0, err
	}
	b.connection = c
	receive := func(buffers [][]byte, sizes []int, endpoints []conn.Endpoint) (int, error) {
		n, address, err := c.ReadFromUDPAddrPort(buffers[0])
		if err != nil {
			return 0, net.ErrClosed
		}
		if n >= 4 && buffers[0][0] >= 1 && buffers[0][0] <= 4 {
			clear(buffers[0][1:4])
		}
		sizes[0] = n
		endpoints[0] = &endpoint{address: address}
		return 1, nil
	}
	return []conn.ReceiveFunc{receive}, uint16(c.LocalAddr().(*net.UDPAddr).Port), nil
}

func (b *udpBind) Send(buffers [][]byte, destination conn.Endpoint) error {
	b.mu.Lock()
	c := b.connection
	b.mu.Unlock()
	if c == nil {
		return net.ErrClosed
	}
	last := len(buffers) - 1
	initiationBatch := last >= 0 && len(buffers[last]) == 148 && buffers[last][0] == 1
	for index, buffer := range buffers {
		// WARP uses the WireGuard reserved header bytes as the registration client ID.
		// AmneziaWG prepends I1/junk to the real initiation in the same Send call.
		if (!initiationBatch || index == last) && len(buffer) >= 4 && buffer[0] >= 1 && buffer[0] <= 4 {
			copy(buffer[1:4], b.reserved[:])
		}
		if _, err := c.WriteToUDPAddrPort(buffer, destination.(*endpoint).address); err != nil {
			return err
		}
	}
	return nil
}

func (b *udpBind) Close() error {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.connection == nil {
		return nil
	}
	err := b.connection.Close()
	b.connection = nil
	return err
}

func (b *udpBind) localAddr() string {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.connection == nil {
		return ""
	}
	return b.connection.LocalAddr().String()
}

func (b *udpBind) ParseEndpoint(value string) (conn.Endpoint, error) {
	address, err := netip.ParseAddrPort(value)
	if err != nil {
		return nil, err
	}
	return &endpoint{address: address}, nil
}

func (*udpBind) SetMark(uint32) error { return nil }

// Probe traffic does not benefit from the default bind's large receive batches.
func (*udpBind) BatchSize() int         { return 1 }
func (*endpoint) ClearSrc()             {}
func (*endpoint) SrcToString() string   { return "" }
func (e *endpoint) DstToString() string { return e.address.String() }
func (e *endpoint) DstToBytes() []byte  { value, _ := e.address.MarshalBinary(); return value }
func (e *endpoint) DstIP() netip.Addr   { return e.address.Addr() }
func (*endpoint) SrcIP() netip.Addr     { return netip.Addr{} }
