package hyperuuid

import (
	"runtime"
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

// The platform's own calls, for the scoreboard: google/uuid's v4 and its v7 (which keeps a
// monotonic sequence of its own), both reading the operating system's random source through
// crypto/rand on every call.

func BenchmarkGoogleUuidNewRandom(b *testing.B) {
	for b.Loop() {
		if _, err := uuid.NewRandom(); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkGoogleUuidNewV7(b *testing.B) {
	for b.Loop() {
		if _, err := uuid.NewV7(); err != nil {
			b.Fatal(err)
		}
	}
}

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

// ---- Contention ---------------------------------------------------------------------------
//
// The stateless calls run on the primary instance when its lock is free and on a pooled
// instance otherwise (core_pool.go); v7 calls always take the primary's lock. These measure
// both under GOMAXPROCS goroutines, with allocations reported, so the README's contention
// numbers are the shipped binding's and the pool's cost shows up as allocations and GCs.

func BenchmarkParallelNewV4(b *testing.B) {
	b.ReportAllocs()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			if _, err := NewV4(); err != nil {
				b.Fatal(err)
			}
		}
	})
}

func BenchmarkParallelNewV7(b *testing.B) {
	b.ReportAllocs()
	b.RunParallel(func(pb *testing.PB) {
		clock := newBenchClock() // one per goroutine: the clock is not shared
		for pb.Next() {
			if _, err := NewV7At(clock.next()); err != nil {
				b.Fatal(err)
			}
		}
	})
}

// BenchmarkParallelMixed is v4, v5 of a 32-byte name, v7 and Version, round-robin on every
// goroutine, with the heap in use and the collections the run cost reported beside the time.
func BenchmarkParallelMixed(b *testing.B) {
	b.ReportAllocs()
	name := []byte("0123456789abcdef0123456789abcdef")
	var before runtime.MemStats
	runtime.ReadMemStats(&before)
	b.RunParallel(func(pb *testing.PB) {
		clock := newBenchClock()
		var last uuid.UUID
		for i := 0; pb.Next(); i++ {
			var err error
			switch i & 3 {
			case 0:
				last, err = NewV4()
			case 1:
				last, err = NewV5(NamespaceDNS, name)
			case 2:
				last, err = NewV7At(clock.next())
			default:
				if Version(last) < 0 {
					b.Fatal("version")
				}
			}
			if err != nil {
				b.Fatal(err)
			}
		}
	})
	var after runtime.MemStats
	runtime.ReadMemStats(&after)
	b.ReportMetric(float64(after.HeapInuse), "heap-inuse-B")
	b.ReportMetric(float64(after.NumGC-before.NumGC), "gcs")
}
