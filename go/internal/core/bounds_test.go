//go:build !tinygo

// TinyGo's wasm targets cannot recover a panic, so these assertions cannot run there.

package core

import (
	"bytes"
	"encoding/binary"
	"runtime"
	"strings"
	"testing"
)

// access is one generated load and store pair at one index type.
type access struct {
	name  string
	width int
	load  func(mem []byte, addr uint64) uint64
	store func(mem []byte, addr, val uint64)
}

func accesses[T uint32 | uint64](index string) []access {
	return []access{
		{
			"16/" + index, 2,
			func(mem []byte, addr uint64) uint64 { return uint64(load16(mem, T(addr))) },
			func(mem []byte, addr, val uint64) { store16(mem, T(addr), uint16(val)) },
		},
		{
			"32/" + index, 4,
			func(mem []byte, addr uint64) uint64 { return uint64(load32(mem, T(addr))) },
			func(mem []byte, addr, val uint64) { store32(mem, T(addr), uint32(val)) },
		},
		{
			"64/" + index, 8,
			func(mem []byte, addr uint64) uint64 { return load64(mem, T(addr)) },
			func(mem []byte, addr, val uint64) { store64(mem, T(addr), val) },
		},
	}
}

func everyAccess() []access {
	return append(accesses[uint32]("uint32"), accesses[uint64]("uint64")...)
}

func panics(f func()) (panicked bool) {
	defer func() { panicked = recover() != nil }()
	f()
	return false
}

func TestAccessPastTheEndPanics(t *testing.T) {
	for _, a := range everyAccess() {
		t.Run(a.name, func(t *testing.T) {
			mem := make([]byte, 64)
			last := uint64(len(mem) - a.width)
			if panics(func() { a.load(mem, last) }) {
				t.Error("a load ending on the last byte panicked")
			}
			if panics(func() { a.store(mem, last, 0) }) {
				t.Error("a store ending on the last byte panicked")
			}
			if !panics(func() { a.load(mem, last+1) }) {
				t.Error("a load straddling the end did not panic")
			}
			for i := range mem {
				mem[i] = byte(i + 1)
			}
			before := bytes.Clone(mem)
			if !panics(func() { a.store(mem, last+1, ^uint64(0)) }) {
				t.Error("a store straddling the end did not panic")
			}
			if !bytes.Equal(mem, before) {
				t.Error("a store that panicked changed the memory")
			}
		})
	}
}

// An address near 2^32 must not wrap to a small one: each of these is a valid offset modulo 2^32.
// Generated code forms an address as a 32-bit pointer plus a small offset, so it never comes near
// 2^64, where the helpers' own check would wrap.
func TestAddressNearFourGiBPanics(t *testing.T) {
	cases := []struct {
		name  string
		addr  uint64
		every []access
	}{
		{"uint32 0xFFFFFFFF", 0xFFFFFFFF, accesses[uint32]("uint32")},
		{"uint64 0xFFFFFFFF", 0xFFFFFFFF, accesses[uint64]("uint64")},
		{"uint64 1<<32", 1 << 32, accesses[uint64]("uint64")},
	}
	for _, c := range cases {
		for _, a := range c.every {
			t.Run(c.name+"/"+a.name, func(t *testing.T) {
				mem := make([]byte, 64)
				if !panics(func() { a.load(mem, c.addr) }) {
					t.Error("the load did not panic")
				}
				if !panics(func() { a.store(mem, c.addr, ^uint64(0)) }) {
					t.Error("the store did not panic")
				}
				if !bytes.Equal(mem, make([]byte, 64)) {
					t.Error("a store that panicked changed the memory")
				}
			})
		}
	}
}

// The guest's memory is little-endian whatever the host, at any alignment.
func TestEveryOffsetRoundTrips(t *testing.T) {
	value := uint64(0x0102030405060708)
	for _, a := range everyAccess() {
		t.Run(a.name, func(t *testing.T) {
			var want []byte
			var truncated uint64
			switch a.width {
			case 2:
				truncated = uint64(uint16(value))
				want = binary.LittleEndian.AppendUint16(nil, uint16(value))
			case 4:
				truncated = uint64(uint32(value))
				want = binary.LittleEndian.AppendUint32(nil, uint32(value))
			default:
				truncated = value
				want = binary.LittleEndian.AppendUint64(nil, value)
			}
			for offset := range uint64(9) {
				mem := make([]byte, 64)
				a.store(mem, offset, value)
				if got := a.load(mem, offset); got != truncated {
					t.Errorf("offset %d: load returned %#x, stored %#x", offset, got, truncated)
				}
				if got := mem[offset : offset+uint64(a.width)]; !bytes.Equal(got, want) {
					t.Errorf("offset %d: memory holds % x, want % x", offset, got, want)
				}
			}
		})
	}
}

func TestAccessAfterGrowth(t *testing.T) {
	mem := make([]byte, 65536)
	addr := uint32(len(mem))
	if !panics(func() { load32(mem, addr) }) {
		t.Fatal("a load past the end did not panic before growth")
	}
	if old := memory_grow(&mem, 1, 4); old != 1 {
		t.Fatalf("memory_grow returned %d, want the old size of 1 page", old)
	}
	store32(mem, addr, 0xCAFEF00D)
	if got := load32(mem, addr); got != 0xCAFEF00D {
		t.Fatalf("load after growth returned %#x", got)
	}
}

// Not uuid_get_timestamp: it checks its version argument and returns a code before it reads.
func TestAnExportPastTheEndPanics(t *testing.T) {
	const past = int32(0x7fffffff)
	defer func() {
		err, ok := recover().(runtime.Error)
		if !ok || !strings.Contains(err.Error(), "out of range") {
			t.Fatalf("the export gave no bounds panic on a pointer past the end of memory: %v", err)
		}
	}()
	New(nil).Xuuid_v7_unix_millis(past)
}
