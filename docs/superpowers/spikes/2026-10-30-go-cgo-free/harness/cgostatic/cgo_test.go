//go:build hyperuuid_spike

package cgostatic

import "testing"

const ms = 1_760_000_000_000

var sink uint64
var out [16]byte

func TestSanity(t *testing.T) {
	u := NewV7(ms)
	if Version() == 0 || V7Millis(u) != ms || u[6]>>4 != 7 {
		t.Fatalf("cgo bad %x", u)
	}
	t.Logf("version %x", Version())
}
func BenchmarkCgoVersion(b *testing.B) {
	for b.Loop() {
		sink += uint64(Version())
	}
}
func BenchmarkCgoNewV4(b *testing.B) {
	for b.Loop() {
		out = NewV4()
	}
}
func BenchmarkCgoNewV4Ptr(b *testing.B) {
	for b.Loop() {
		out = NewV4Ptr()
	}
}
func BenchmarkCgoNewV7(b *testing.B) {
	for b.Loop() {
		out = NewV7(ms)
	}
}
func BenchmarkCgoV7Millis(b *testing.B) {
	u := NewV7(ms)
	for b.Loop() {
		sink += V7Millis(u)
	}
}
func BenchmarkCgoV7Batch1000(b *testing.B) {
	buf := make([]byte, 16000)
	for b.Loop() {
		Batch(ms, 1000, buf)
	}
}

func BenchmarkParallelCgoNewV7(b *testing.B) {
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			NewV7(ms)
		}
	})
}

func BenchmarkParallelCgoNewV4(b *testing.B) {
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			NewV4()
		}
	})
}
