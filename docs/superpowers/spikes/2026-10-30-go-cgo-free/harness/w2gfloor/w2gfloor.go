//go:build hyperuuid_spike

// Package w2gfloor is not a random source: random_get leaves guest memory as it is, so a
// batch costs only the translated walk and the copy out. It is the floor every random source
// adds to, and walk-ab.sh measures the batch walk with it.
package w2gfloor

import (
	"sync"

	"spike/w2gfloor/core"
)

type wasi struct{}

func (wasi) Init(any)                       {}
func (wasi) Xrandom_get(ptr, n int32) int32 { return 0 }

type Core struct {
	mu      sync.Mutex
	m       *core.Module
	scratch uint32
}

func New() *Core {
	m := core.New(wasi{})
	old := m.Xmemory().Grow(1, 65536)
	if old < 0 {
		panic("grow")
	}
	return &Core{m: m, scratch: uint32(old) * 65536}
}

func (c *Core) Batch(ms uint64, n uint32, dst []byte) int32 {
	c.mu.Lock()
	code := c.m.Xuuid_new_v7_batch(int64(ms), int32(n), int32(c.scratch))
	copy(dst, (*c.m.Xmemory().Slice())[c.scratch:c.scratch+16*n])
	c.mu.Unlock()
	return code
}
