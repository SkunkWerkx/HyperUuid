package hyperuuid

// Portable tests of the translated-core backend: no build tag and no os/exec, so they run
// under stock Go on every port, under TinyGo and in the browser page's runtimes.

import (
	"bytes"
	"crypto/rand"
	"errors"
	"math"
	"slices"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/google/uuid"
)

// withFreshInstances replaces the primary with a new instance and the pool with an empty one
// (no call may be in flight), returns the instance count after that, and restores both on
// cleanup. Tests assert deltas against the returned baseline, never absolute counts.
func withFreshInstances(t testing.TB) (baseline int64) {
	t.Helper()
	old := primary()
	fresh, ok := newInstance(basePages)
	if !ok {
		t.Fatal("a fresh instance could not grow its base page")
	}
	thePrimary = fresh
	resetPool()
	t.Cleanup(func() {
		thePrimary = old
		resetPool()
	})
	return instancesCreated.Load()
}

// v7Counter is the 26-bit counter as rust/src/v7.rs writes it across octets 6-9, around the
// version nibble and the variant bits.
func v7Counter(u uuid.UUID) uint32 {
	return uint32(u[6]&0x0f)<<22 | uint32(u[7])<<14 | uint32(u[8]&0x3f)<<8 | uint32(u[9])
}

func TestV7Counter(t *testing.T) {
	cases := []struct {
		octets [4]byte
		want   uint32
	}{
		{[4]byte{0x7f, 0xff, 0xbf, 0xff}, 1<<26 - 1},
		{[4]byte{0x70, 0x00, 0x80, 0x00}, 0},
		{[4]byte{0x70, 0x01, 0x80, 0x00}, 1 << 14},
	}
	for _, c := range cases {
		var u uuid.UUID
		copy(u[6:10], c.octets[:])
		if got := v7Counter(u); got != c.want {
			t.Errorf("octets %x: counter %d, want %d", c.octets, got, c.want)
		}
	}
}

func TestScratchPagesFor(t *testing.T) {
	cases := []struct {
		bytes uint64
		pages uint32
		ok    bool
	}{
		{0, 0, true},
		{1, 1, true},
		{65536, 1, true},
		{65537, 2, true},
		{uint64(hostMaxPages-2) * 65536, hostMaxPages - 2, true},
		{uint64(hostMaxPages-2)*65536 + 1, 0, false},
		{math.MaxUint64, 0, false},
	}
	for _, c := range cases {
		pages, ok := scratchPagesFor(c.bytes)
		if ok != c.ok || (ok && pages != c.pages) {
			t.Errorf("scratchPagesFor(%d) = %d, %v; want %d, %v", c.bytes, pages, ok, c.pages, c.ok)
		}
	}
}

func TestMaxV5NameIsTheSmallerBound(t *testing.T) {
	if strconv.IntSize == 64 && MaxV5Name != 1<<31-1 {
		t.Fatalf("MaxV5Name = %d on a 64-bit host, want %d", MaxV5Name, 1<<31-1)
	}
	if _, ok := scratchPagesFor(uint64(MaxV5Name) + 32); !ok {
		t.Fatal("a name of MaxV5Name bytes does not fit the scratch")
	}
}

func TestV5NameAtThePageBoundary(t *testing.T) {
	withFreshInstances(t)
	ns := NamespaceDNS
	for _, n := range []int{65504, 65505} {
		name := bytes.Repeat([]byte{byte(n)}, n)
		got, err := NewV5(ns, name)
		if err != nil {
			t.Fatalf("NewV5 of %d bytes: %v", n, err)
		}
		if want := uuid.NewSHA1(ns, name); got != want {
			t.Fatalf("NewV5 of %d bytes = %s, want %s", n, got, want)
		}
		if pages := primary().pages; pages != basePages {
			t.Fatalf("after a v5 of %d bytes the primary has %d scratch pages, want %d", n, pages, basePages)
		}
	}
}

func TestNewV5PastTheLimit(t *testing.T) {
	if _, err := NewV5(NamespaceDNS, nil); err != nil {
		t.Fatalf("NewV5 of a nil name: %v", err)
	}
	if err := checkV5Name(MaxV5Name); err != nil {
		t.Fatalf("checkV5Name(MaxV5Name) = %v", err)
	}
	err := checkV5Name(MaxV5Name + 1)
	if !errors.Is(err, ErrNameTooLong) {
		t.Fatalf("checkV5Name(MaxV5Name+1) = %v, want ErrNameTooLong", err)
	}
	for _, n := range []int{MaxV5Name + 1, MaxV5Name} {
		if !strings.Contains(err.Error(), strconv.Itoa(n)) {
			t.Fatalf("%q does not name %d", err, n)
		}
	}
}

func TestBatchGrowsAndReusesTheScratch(t *testing.T) {
	withFreshInstances(t)
	for i := range 2 {
		if _, err := NewV7Batch(5000); err != nil {
			t.Fatalf("batch %d: %v", i, err)
		}
		if pages := primary().pages; pages != 2 {
			t.Fatalf("after batch %d the primary has %d scratch pages, want 2", i, pages)
		}
	}
}

func TestV5DrawsNoEntropy(t *testing.T) {
	withFreshInstances(t)
	r := &countingReader{}
	withEntropy(t, r)
	if _, err := NewV5(NamespaceURL, make([]byte, 1024)); err != nil {
		t.Fatal(err)
	}
	if r.calls != 0 {
		t.Fatalf("a v5 read the entropy source %d times", r.calls)
	}
}

func TestBatchGrowWhileStatelessCallsRun(t *testing.T) {
	done := make(chan struct{})
	var wg sync.WaitGroup
	for i := range 8 {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			for {
				select {
				case <-done:
					return
				default:
				}
				if i%2 == 0 {
					if _, err := NewV4(); err != nil {
						t.Error(err)
						return
					}
				} else if v := Version(Nil); v != 0 {
					t.Errorf("Version(Nil) = %d", v)
					return
				}
			}
		}(i)
	}
	for _, n := range []int{1000, 50000, 1000} {
		if _, err := NewV7Batch(n); err != nil {
			t.Fatalf("batch of %d: %v", n, err)
		}
	}
	close(done)
	wg.Wait()
}

func TestV7StaysOrderedAcrossGoroutines(t *testing.T) {
	ms := uint64(time.Now().UnixMilli())
	var wg sync.WaitGroup
	for range 12 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			var prev uuid.UUID
			for i := range 10000 {
				u, err := NewV7At(ms)
				if err != nil {
					t.Error(err)
					return
				}
				if i > 0 && bytes.Compare(u[:], prev[:]) <= 0 {
					t.Errorf("v7 %s does not sort after %s", u, prev)
					return
				}
				prev = u
			}
		}()
	}
	wg.Wait()
}

func TestEntropyFailureReachesTheCaller(t *testing.T) {
	withAccelerated(t, false) // selection happens at instance creation
	withFreshInstances(t)
	withEntropy(t, &failingReader{ok: 3, err: errors.New("entropy gone")})
	if _, err := NewV4(); !errors.Is(err, ErrRandomSource) {
		t.Fatalf("NewV4 with a failing reader = %v, want ErrRandomSource", err)
	}
}

func TestBatchFailureOnALaterChunkReservesNothing(t *testing.T) {
	withAccelerated(t, false)
	withFreshInstances(t)
	ms := uint64(time.Now().UnixMilli())
	single := func() uint32 {
		t.Helper()
		u, err := NewV7At(ms)
		if err != nil {
			t.Fatal(err)
		}
		return v7Counter(u)
	}
	c0 := single()
	working := entropy
	withEntropy(t, &failingReader{ok: 70000, err: errors.New("entropy gone")})
	if _, err := NewV7BatchAt(20_000, ms); !errors.Is(err, ErrRandomSource) {
		t.Fatalf("a batch whose second chunk fails = %v, want ErrRandomSource", err)
	}
	withEntropy(t, working)
	c1 := single()
	if d := c1 - c0; d < 1 || d > 127 {
		t.Fatalf("the failed batch moved the counter by %d: it reserved steps", d)
	}
	// Control: a batch that succeeds reserves ⌈20,000/64⌉ = 313 steps of 64.
	if _, err := NewV7BatchAt(20_000, ms); err != nil {
		t.Fatal(err)
	}
	if d := single() - c1; d < 313*64 {
		t.Fatalf("a successful batch of 20,000 moved the counter by only %d, want at least %d", d, 313*64)
	}
}

// recordingReader serves random bytes and records the size of every read: with the OS source
// one read is one random_get, so the sizes are the core's draws.
type recordingReader struct{ sizes []int }

func (r *recordingReader) Read(p []byte) (int, error) {
	r.sizes = append(r.sizes, len(p))
	return rand.Read(p)
}

// A batch draws its entropy up front, 6 bytes an item and 4 to start the counter, in one
// random_get up to the core's 64 KiB chunk and in 64 KiB chunks past it: the premise of the
// stream's 1 MiB budget check inside a draw and of the README's account of a batch.
func TestBatchDrawsArriveIn64KiBChunks(t *testing.T) {
	withAccelerated(t, false)
	withFreshInstances(t)
	rec := &recordingReader{}
	withEntropy(t, rec)
	ms := uint64(time.Now().UnixMilli())
	if _, err := NewV7BatchAt(1000, ms); err != nil {
		t.Fatal(err)
	}
	if want := []int{6004}; !slices.Equal(rec.sizes, want) {
		t.Fatalf("a batch of 1000 drew %v, want %v", rec.sizes, want)
	}
	rec.sizes = nil
	if _, err := NewV7BatchAt(20_000, ms); err != nil {
		t.Fatal(err)
	}
	if want := []int{65536, 120004 - 65536}; !slices.Equal(rec.sizes, want) {
		t.Fatalf("a batch of 20,000 drew %v, want %v", rec.sizes, want)
	}
}
