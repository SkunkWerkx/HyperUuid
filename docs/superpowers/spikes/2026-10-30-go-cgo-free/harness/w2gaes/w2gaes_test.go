//go:build hyperuuid_spike

package w2gaes

import "testing"

const ms = 1_760_000_000_000

var (
	sink uint64
	out  [16]byte
	c    = New()
)

func TestSanity(t *testing.T) {
	u, code := c.NewV7(ms)
	if c.Version() == 0 || code != 0 || c.V7Millis(u) != ms || u[6]>>4 != 7 {
		t.Fatalf("w2g bad code=%d %x", code, u)
	}
	a, _ := c.NewV4()
	b, _ := c.NewV4()
	if a == b || a[6]>>4 != 4 || a[8]>>6 != 2 {
		t.Fatalf("v4 bad %x %x", a, b)
	}
	buf := make([]byte, 16000)
	if c.Batch(ms, 1000, buf) != 0 {
		t.Fatal("batch")
	}
	for i := 1; i < 1000; i++ {
		if string(buf[16*(i-1):16*i]) >= string(buf[16*i:16*(i+1)]) {
			t.Fatalf("batch not increasing at %d", i)
		}
	}
	t.Logf("version %x v4 %x", c.Version(), a)
}

func BenchmarkW2gVersion(b *testing.B) {
	for b.Loop() {
		sink += uint64(c.Version())
	}
}
func BenchmarkW2gNewV4(b *testing.B) {
	for b.Loop() {
		out, _ = c.NewV4()
	}
}
func BenchmarkW2gNewV7(b *testing.B) {
	for b.Loop() {
		out, _ = c.NewV7(ms)
	}
}
func BenchmarkW2gV7Millis(b *testing.B) {
	u, _ := c.NewV7(ms)
	for b.Loop() {
		sink += c.V7Millis(u)
	}
}
func BenchmarkW2gV7Batch1000(b *testing.B) {
	buf := make([]byte, 16000)
	for b.Loop() {
		c.Batch(ms, 1000, buf)
	}
}
func BenchmarkParallelW2gNewV7(b *testing.B) {
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			c.NewV7(ms)
		}
	})
}

func BenchmarkParallelW2gNewV4(b *testing.B) {
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			c.NewV4()
		}
	})
}
