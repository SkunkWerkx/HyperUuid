//go:build hyperuuid_spike

package w2gfloor

import "testing"

const ms = 1_760_000_000_000

var c = New()

// Only the batch: with no entropy a single call's numbers say nothing about any random source.
func BenchmarkW2gV7Batch1000(b *testing.B) {
	buf := make([]byte, 16000)
	for b.Loop() {
		c.Batch(ms, 1000, buf)
	}
}
