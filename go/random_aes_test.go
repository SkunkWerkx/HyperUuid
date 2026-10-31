//go:build (amd64 || arm64 || s390x) && !purego && !tinygo

package hyperuuid

import (
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"errors"
	"io"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"
)

// keystream is the AES-256-CTR keystream of key under a zero IV: the bytes the stream must
// serve, in order, from offset 0.
func keystream(key []byte, n int) []byte {
	block, err := aes.NewCipher(key)
	if err != nil {
		panic(err)
	}
	out := make([]byte, n)
	cipher.NewCTR(block, make([]byte, 16)).XORKeyStream(out, out)
	return out
}

func testKey(seed byte) []byte {
	key := make([]byte, 32)
	for i := range key {
		key[i] = seed + byte(i)*13
	}
	return key
}

// A step of a scriptReader: data served in order, then err if set.
type step struct {
	data []byte
	err  error
}

// scriptReader serves its steps in order: each step's bytes, then its error if it has one,
// then the next step. A reader that fails partway through a key and later succeeds is a
// script of three steps.
type scriptReader struct{ steps []step }

func (r *scriptReader) Read(p []byte) (int, error) {
	for len(r.steps) > 0 {
		s := &r.steps[0]
		if len(s.data) > 0 {
			n := copy(p, s.data)
			s.data = s.data[n:]
			return n, nil
		}
		r.steps = r.steps[1:]
		if s.err != nil {
			return 0, s.err
		}
	}
	return 0, io.EOF
}

type clock struct{ t time.Time }

func (c *clock) now() time.Time { return c.t }

// newTestStream returns a stream whose keys come from the reader's steps and whose clock
// the test advances.
func newTestStream(t *testing.T, steps ...step) (*aesSource, *clock) {
	t.Helper()
	withEntropy(t, &scriptReader{steps: steps})
	c := &clock{t: time.Unix(1_700_000_000, 0)}
	return newAESSource(c.now), c
}

func draw(t *testing.T, s *aesSource, n int) []byte {
	t.Helper()
	dst := make([]byte, n)
	if code := s.fill(dst); code != 0 {
		t.Fatalf("fill(%d) returned %d", n, code)
	}
	return dst
}

func TestStreamIsOneContiguousKeystream(t *testing.T) {
	k1 := testKey(1)
	s, _ := newTestStream(t, step{data: k1})
	want := keystream(k1, 2032)
	a, b, c := draw(t, s, 16), draw(t, s, 2000), draw(t, s, 16)
	if !bytes.Equal(a, want[:16]) {
		t.Errorf("first 16 differ from offsets 0-15")
	}
	if !bytes.Equal(b, want[16:2016]) {
		t.Errorf("the 2000 differ from offsets 16-2015")
	}
	if !bytes.Equal(c, want[2016:2032]) {
		t.Errorf("last 16 differ from offsets 2016-2031")
	}
}

func TestDirectGenerationZeroesTheDestination(t *testing.T) {
	k1 := testKey(2)
	s, _ := newTestStream(t, step{data: k1})
	dst := bytes.Repeat([]byte{0xFF}, 2000)
	if code := s.fill(dst); code != 0 {
		t.Fatalf("fill returned %d", code)
	}
	if !bytes.Equal(dst, keystream(k1, 2000)) {
		t.Fatal("a prefilled destination is not the keystream: the direct path did not zero it")
	}
}

func TestTimeExpiryBeforeServingBufferedBytes(t *testing.T) {
	k1, k2 := testKey(3), testKey(4)
	s, c := newTestStream(t, step{data: k1}, step{data: k2})
	draw(t, s, 16)
	c.t = c.t.Add(2 * time.Second)
	got := draw(t, s, 16)
	if !bytes.Equal(got, keystream(k2, 16)) {
		t.Fatal("after expiry the next bytes are not the new key's offsets 0-15")
	}
	if bytes.Equal(got, keystream(k1, 32)[16:]) {
		t.Fatal("offsets 16-31 of the expired key were served")
	}
}

func TestSizeBudgetSplitsAGeneration(t *testing.T) {
	k1, k2 := testKey(5), testKey(6)
	s, _ := newTestStream(t, step{data: k1}, step{data: k2})
	draw(t, s, 16) // keyed with k1, 512 generated into the buffer
	s.generated = keyBudget - 100
	s.off = bufSize // the buffer is empty: the draw is direct
	got := draw(t, s, 1024)
	first := keystream(k1, 612)[512:] // the stream is at offset 512 after the refill
	if !bytes.Equal(got[:100], first) {
		t.Error("the first 100 bytes are not the first key's next keystream bytes")
	}
	if !bytes.Equal(got[100:], keystream(k2, 924)) {
		t.Error("the remaining 924 bytes are not the second key's first keystream bytes")
	}
	if s.generated != 924 {
		t.Errorf("generated = %d after the split, want 924", s.generated)
	}
}

func TestBudgetCountsRefillsAndDirect(t *testing.T) {
	s, _ := newTestStream(t, step{data: testKey(7)})
	draw(t, s, 16)
	if s.generated != 512 {
		t.Fatalf("generated = %d after one refill, want 512", s.generated)
	}
	draw(t, s, 2000)
	if s.generated != 2016 {
		t.Fatalf("generated = %d after 496 drained and 1504 direct, want 2016", s.generated)
	}
}

func TestFailedRekeyLeavesTheSourceKeyless(t *testing.T) {
	k1, k2 := testKey(8), testKey(9)
	s, c := newTestStream(t,
		step{data: k1},
		step{data: k2[:10], err: errors.New("entropy gone")},
		step{data: k2},
	)
	draw(t, s, 16)
	c.t = c.t.Add(2 * time.Second)
	dst := make([]byte, 16)
	if code := s.fill(dst); code != wasiEIO {
		t.Fatalf("fill after a failed rekey returned %d, want %d", code, wasiEIO)
	}
	if s.stream != nil {
		t.Fatal("a failed rekey left a stream in place")
	}
	got := draw(t, s, 16)
	if !bytes.Equal(got, keystream(k2, 16)) {
		t.Fatal("after recovery the bytes are not the new key's offsets 0-15")
	}
	if bytes.Equal(got, keystream(k1, 32)[16:]) {
		t.Fatal("the old buffer was served after a failed rekey")
	}
}

func TestFailedRekeyAtTheByteBoundary(t *testing.T) {
	k1 := testKey(10)
	s, _ := newTestStream(t,
		step{data: k1},
		step{data: []byte{1, 2, 3, 4, 5}, err: errors.New("entropy gone")},
	)
	draw(t, s, 16)
	s.generated = keyBudget - 100
	s.off = bufSize
	dst := make([]byte, 1024)
	if code := s.fill(dst); code != wasiEIO {
		t.Fatalf("a draw whose mid-draw rekey failed returned %d, want %d", code, wasiEIO)
	}
	if s.stream != nil {
		t.Fatal("a failed mid-draw rekey left a stream in place")
	}
}

func TestRekeyAllocations(t *testing.T) {
	withEntropy(t, rand.Reader)
	c := &clock{t: time.Unix(1_700_000_000, 0)}
	s := newAESSource(c.now)
	rekeys := testing.AllocsPerRun(10, func() {
		if !s.rekey() {
			t.Fatal("rekey failed with an endless reader")
		}
	})
	t.Logf("allocations per rekey (aes.NewCipher + cipher.NewCTR): %.0f", rekeys)
	dst := make([]byte, 16)
	if !s.rekey() {
		t.Fatal("rekey failed")
	}
	if fills := testing.AllocsPerRun(100, func() { s.fill(dst) }); fills != 0 {
		t.Fatalf("fill of 16 between rekeys allocates %.0f, want 0", fills)
	}
}

func probeSelection(t *testing.T, env ...string) string {
	t.Helper()
	cmd := exec.Command(os.Args[0], "-test.run", "^TestSelectionProbe$", "-test.v")
	cmd.Env = append(os.Environ(), "HYPERUUID_SELECTION_PROBE=1")
	cmd.Env = append(cmd.Env, env...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("probe failed: %v\n%s", err, out)
	}
	for _, line := range strings.Split(string(out), "\n") {
		if i := strings.Index(line, "selection="); i >= 0 {
			return strings.TrimSpace(line[i+len("selection="):])
		}
	}
	t.Fatalf("probe printed no selection:\n%s", out)
	return ""
}

func TestSelectionUnderGODEBUG(t *testing.T) {
	if !accelerated() {
		t.Skip("this CPU does not accelerate AES; the GODEBUG control needs a machine that does")
	}
	if got := probeSelection(t); got != "stream" {
		t.Fatalf("default selection on an accelerated CPU is %q, want stream", got)
	}
	if got := probeSelection(t, "GODEBUG=cpu.aes=off"); got != "os" {
		t.Fatalf("selection under GODEBUG=cpu.aes=off is %q, want os", got)
	}
}

func TestSelectionUnderFIPS(t *testing.T) {
	if !accelerated() {
		t.Skip("this CPU does not accelerate AES; the FIPS control needs a machine that does")
	}
	if got := probeSelection(t, "GODEBUG=fips140=on"); got != "os" {
		t.Fatalf("selection under GODEBUG=fips140=on is %q, want os", got)
	}
}
