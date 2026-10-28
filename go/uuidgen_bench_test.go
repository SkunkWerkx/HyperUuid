package hyperuuid

import (
	"testing"
	"time"

	"github.com/google/uuid"
)

// Run with: go test -bench=. -benchmem ./...
// -benchmem is essentially free here — allocation tracking is built into testing.B, no
// extra tooling required, unlike the other bindings.

func BenchmarkNewV4(b *testing.B) {
	for b.Loop() {
		if _, err := NewV4(); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkNewV5String(b *testing.B) {
	for b.Loop() {
		if _, err := NewV5String(NamespaceDNS, "www.example.com"); err != nil {
			b.Fatal(err)
		}
	}
}

// benchClock hands the v7 generation benchmarks the current Unix millisecond, re-read from the
// clock once every 1000 calls, so the clock read stays out of the per-call figure and every call
// is minted through the stream. The stream was moved to the present when the test binary
// started (stream_prime_test.go), so a fixed timestamp from the past would be more than 11 s
// behind it and time the out-of-stream path instead, and one fixed timestamp held long enough
// would split the stream forward until it fails with ErrCounterExhausted.
type benchClock struct {
	ms    uint64
	calls int
}

func newBenchClock() *benchClock {
	return &benchClock{ms: uint64(time.Now().UnixMilli())}
}

func (c *benchClock) next() uint64 {
	c.calls++
	if c.calls == 1000 {
		c.calls = 0
		c.ms = uint64(time.Now().UnixMilli())
	}
	return c.ms
}

func BenchmarkNewV7At(b *testing.B) {
	clock := newBenchClock()
	for b.Loop() {
		if _, err := NewV7At(clock.next()); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkV7IndividualX1000(b *testing.B) {
	clock := newBenchClock()
	for b.Loop() {
		for i := 0; i < 1000; i++ {
			if _, err := NewV7At(clock.next()); err != nil {
				b.Fatal(err)
			}
		}
	}
}

func BenchmarkNewV7BatchAt1000(b *testing.B) {
	clock := newBenchClock()
	for b.Loop() {
		if _, err := NewV7BatchAt(1000, clock.next()); err != nil {
			b.Fatal(err)
		}
	}
}

// Extraction benchmarks: google/uuid is already a real dependency here and has its own
// timestamp-extraction method (uuid.UUID.Time(), documented as defined for versions 1, 2, 6,
// and 7) — these measure HyperUuid's V7Timestamp head-to-head against it on the
// same pre-generated UUID, generation excluded from the timed loop.

func BenchmarkV7TimestampExtraction(b *testing.B) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		b.Fatal(err)
	}
	for b.Loop() {
		if _, err := V7Timestamp(id); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkGoogleUuidV7TimeExtraction(b *testing.B) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		b.Fatal(err)
	}
	for b.Loop() {
		_ = id.Time()
	}
}

// Fill vs NewBatch: same native work, but writing into a slice the caller already owns
// instead of allocating a fresh one per call. Unlike the C# binding, the []uuid.UUID form
// needs no per-element conversion either — uuid.UUID is [16]byte, so the native core writes
// the whole batch straight into the destination.
func BenchmarkFillV7At1000(b *testing.B) {
	dst := make([]uuid.UUID, 1000)
	clock := newBenchClock()
	for b.Loop() {
		if err := FillV7At(dst, clock.next()); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkFillV7BytesAt1000(b *testing.B) {
	dst := make([]byte, 1000*16)
	clock := newBenchClock()
	for b.Loop() {
		if err := FillV7BytesAt(dst, clock.next()); err != nil {
			b.Fatal(err)
		}
	}
}
