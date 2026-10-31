package hyperuuid

import (
	"bytes"
	"errors"
	"io"
	"os"
	"testing"
	"time"
)

// withEntropy swaps the package's checked reader for r and restores it on cleanup, so an
// injected reader never leaks into another test or into a stream's rekey elsewhere.
func withEntropy(t testing.TB, r io.Reader) {
	t.Helper()
	old := entropy
	entropy = r
	t.Cleanup(func() { entropy = old })
}

// withAccelerated forces accelerated() to on for the test and restores it on cleanup.
func withAccelerated(t testing.TB, on bool) {
	t.Helper()
	old := acceleratedOverride
	acceleratedOverride = func() bool { return on }
	t.Cleanup(func() { acceleratedOverride = old })
}

// shortReader delivers n bytes per Read with a nil error, from seq.
type shortReader struct {
	seq []byte
	n   int
}

func (r *shortReader) Read(p []byte) (int, error) {
	n := min(r.n, len(p), len(r.seq))
	copy(p, r.seq[:n])
	r.seq = r.seq[n:]
	return n, nil
}

// failingReader serves ok bytes, then returns err.
type failingReader struct {
	ok  int
	err error
}

func (r *failingReader) Read(p []byte) (int, error) {
	if r.ok == 0 {
		return 0, r.err
	}
	n := min(r.ok, len(p))
	for i := range n {
		p[i] = 0xAB
	}
	r.ok -= n
	return n, nil
}

// countingReader counts Read calls and serves zeros.
type countingReader struct{ calls int }

func (r *countingReader) Read(p []byte) (int, error) {
	r.calls++
	clear(p)
	return len(p), nil
}

func sequence(n int) []byte {
	seq := make([]byte, n)
	for i := range seq {
		seq[i] = byte(i*7 + 3)
	}
	return seq
}

func TestOSSourceReadsThroughTheCheckedReader(t *testing.T) {
	seq := sequence(64)
	withEntropy(t, bytes.NewReader(seq))
	dst := make([]byte, 40)
	if code := (osSource{}).fill(dst); code != 0 {
		t.Fatalf("fill returned %d", code)
	}
	if !bytes.Equal(dst, seq[:len(dst)]) {
		t.Fatalf("dst = %x, want %x", dst, seq[:len(dst)])
	}
}

func TestShortSuccessfulReadsAreRetried(t *testing.T) {
	seq := sequence(16)
	withEntropy(t, &shortReader{seq: seq, n: 3})
	dst := make([]byte, 16)
	if code := (osSource{}).fill(dst); code != 0 {
		t.Fatalf("fill returned %d", code)
	}
	if !bytes.Equal(dst, seq) {
		t.Fatalf("dst = %x, want %x", dst, seq)
	}
}

func TestFailingReaderIsEIO(t *testing.T) {
	withEntropy(t, &failingReader{ok: 5, err: errors.New("x")})
	dst := make([]byte, 16)
	if code := (osSource{}).fill(dst); code != wasiEIO {
		t.Fatalf("fill returned %d, want %d", code, wasiEIO)
	}
}

func TestEOFReaderIsEIO(t *testing.T) {
	withEntropy(t, bytes.NewReader(make([]byte, 5)))
	dst := make([]byte, 16)
	if code := (osSource{}).fill(dst); code != wasiEIO {
		t.Fatalf("fill returned %d, want %d", code, wasiEIO)
	}
}

func TestZeroLengthDrawTouchesNothing(t *testing.T) {
	r := &countingReader{}
	withEntropy(t, r)
	if code := (osSource{}).fill(nil); code != 0 {
		t.Fatalf("fill returned %d", code)
	}
	if r.calls != 0 {
		t.Fatalf("reader called %d times for a zero-length draw", r.calls)
	}
}

func TestSelectionWithoutAcceleration(t *testing.T) {
	withAccelerated(t, false)
	if _, ok := newSource(time.Now).(osSource); !ok {
		t.Fatalf("newSource without acceleration is %T, want osSource", newSource(time.Now))
	}
}

// TestSelectionProbe logs the selected source when HYPERUUID_SELECTION_PROBE is set; the
// selection tests run it in a child process under GODEBUG settings, and the purego build
// runs it directly. It lives in this untagged file so that every build has it.
func TestSelectionProbe(t *testing.T) {
	if os.Getenv("HYPERUUID_SELECTION_PROBE") == "" {
		t.Skip("probe: set HYPERUUID_SELECTION_PROBE=1")
	}
	if _, ok := newSource(time.Now).(osSource); ok {
		t.Log("selection=os")
	} else {
		t.Log("selection=stream")
	}
}
