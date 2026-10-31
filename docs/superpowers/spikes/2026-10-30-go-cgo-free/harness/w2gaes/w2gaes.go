//go:build hyperuuid_spike

package w2gaes

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/binary"
	"sync"
	"time"

	"spike/w2gaes/core"
)

// SPIKE: random_get served from an AES-256-CTR stream (crypto/aes, on the CPU's AES
// instructions where it has them) keyed from crypto/rand, rekeyed after 1 MiB or one second
// (the core's own pool rule). Each key is fresh, so the counter starts at zero. Calls are
// already serialized by Core.mu.
type wasi struct {
	m       *core.Module
	g       cipher.Stream
	used    int
	keyedAt time.Time
}

func (w *wasi) rekey() {
	var key [32]byte
	var iv [aes.BlockSize]byte
	rand.Read(key[:])
	block, _ := aes.NewCipher(key[:])
	w.g = cipher.NewCTR(block, iv[:])
	w.used, w.keyedAt = 0, time.Now()
}

func (w *wasi) Init(m any) { w.m = m.(*core.Module) }

// random_get(buf, len) -> errno, filled from crypto/rand straight into guest memory.
func (w *wasi) Xrandom_get(ptr, n int32) int32 {
	if w.g == nil || w.used > 1<<20 || time.Since(w.keyedAt) > time.Second {
		w.rekey()
	}
	mem := *w.m.Xmemory().Slice()
	dst := mem[uint32(ptr) : uint32(ptr)+uint32(n)]
	clear(dst)
	w.g.XORKeyStream(dst, dst)
	w.used += int(n)
	return 0
}

type Core struct {
	mu      sync.Mutex
	m       *core.Module
	scratch uint32
}

func New() *Core {
	m := core.New(&wasi{})
	old := m.Xmemory().Grow(1, 65536)
	if old < 0 {
		panic("grow")
	}
	return &Core{m: m, scratch: uint32(old) * 65536}
}

func (c *Core) mem() []byte { return *c.m.Xmemory().Slice() }

func (c *Core) Version() uint32 {
	c.mu.Lock()
	v := uint32(c.m.Xhyperuuid_version())
	c.mu.Unlock()
	return v
}

func (c *Core) NewV4() (u [16]byte, code int32) {
	c.mu.Lock()
	code = c.m.Xuuid_new_v4(int32(c.scratch))
	copy(u[:], c.mem()[c.scratch:])
	c.mu.Unlock()
	return
}

func (c *Core) NewV7(ms uint64) (u [16]byte, code int32) {
	c.mu.Lock()
	code = c.m.Xuuid_new_v7(int64(ms), int32(c.scratch))
	copy(u[:], c.mem()[c.scratch:])
	c.mu.Unlock()
	return
}

func (c *Core) V7Millis(u [16]byte) uint64 {
	c.mu.Lock()
	mem := c.mem()
	binary.LittleEndian.PutUint64(mem[c.scratch:], binary.LittleEndian.Uint64(u[:8]))
	binary.LittleEndian.PutUint64(mem[c.scratch+8:], binary.LittleEndian.Uint64(u[8:]))
	r := uint64(c.m.Xuuid_v7_unix_millis(int32(c.scratch)))
	c.mu.Unlock()
	return r
}

func (c *Core) Batch(ms uint64, n uint32, dst []byte) int32 {
	c.mu.Lock()
	code := c.m.Xuuid_new_v7_batch(int64(ms), int32(n), int32(c.scratch))
	copy(dst, c.mem()[c.scratch:c.scratch+16*n])
	c.mu.Unlock()
	return code
}
