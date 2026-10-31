//go:build (linux || darwin) && !tinygo

package hyperuuid

// The README's memory receipts: for each shape, in a fresh process, the guest memory's
// logical length, what append retained for it, and the process's peak RSS. The only file
// that imports syscall for Getrusage, which Windows, the BSDs, wasm and TinyGo never compile.
// Skips unless HYPERUUID_RECEIPTS=1; nothing else references these symbols.

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

var receiptShapes = []string{"maxbatch", "increments", "maxv5"}

type receipt struct {
	shape                      string
	logical, retained, peakRSS uint64
}

func TestReceipts(t *testing.T) {
	if os.Getenv("HYPERUUID_RECEIPTS") == "" {
		t.Skip("set HYPERUUID_RECEIPTS=1 to measure")
	}
	for _, shape := range receiptShapes {
		cmd := exec.Command(os.Args[0], "-test.run", "^TestReceiptShape$", "-test.v")
		cmd.Env = append(os.Environ(), "HYPERUUID_RECEIPT_SHAPE="+shape)
		out, err := cmd.CombinedOutput()
		if err != nil {
			t.Fatalf("%s: child failed: %v\n%s", shape, err, out)
		}
		r, ok := parseReceipt(out)
		if !ok {
			t.Fatalf("%s: child printed no receipt:\n%s", shape, out)
		}
		if r.retained < r.logical {
			t.Fatalf("%s: retained %d below logical %d", shape, r.retained, r.logical)
		}
		t.Logf("%-10s logical %6d KiB  retained %6d KiB  peak RSS %6d KiB  (ru_maxrss, %s/%s)",
			shape, r.logical>>10, r.retained>>10, r.peakRSS>>10, runtime.GOOS, runtime.GOARCH)
	}
}

func parseReceipt(out []byte) (receipt, bool) {
	for _, line := range bytes.Split(out, []byte("\n")) {
		i := bytes.Index(line, []byte("receipt shape="))
		if i < 0 {
			continue
		}
		var r receipt
		for _, f := range strings.Fields(string(line[i+len("receipt "):])) {
			k, v, _ := strings.Cut(f, "=")
			n, _ := strconv.ParseUint(v, 10, 64)
			switch k {
			case "shape":
				r.shape = v
			case "logical":
				r.logical = n
			case "retained":
				r.retained = n
			case "peak_rss":
				r.peakRSS = n
			}
		}
		return r, r.shape != ""
	}
	return receipt{}, false
}

// TestReceiptShape is the child: one shape, then the three numbers on one line.
func TestReceiptShape(t *testing.T) {
	shape := os.Getenv("HYPERUUID_RECEIPT_SHAPE")
	if shape == "" {
		t.Skip("the child of TestReceipts")
	}
	ms := uint64(time.Now().UnixMilli())
	var mem []byte
	switch shape {
	case "maxbatch":
		if _, err := NewV7BatchAt(MaxV7Batch, ms); err != nil {
			t.Fatal(err)
		}
		mem = primary().mem()
	case "increments":
		// 16 MiB of scratch, then one page more: the small incremental grow of a large slice.
		for _, n := range []int{1 << 20, 1<<20 + 4096} {
			if _, err := NewV7BatchAt(n, ms); err != nil {
				t.Fatal(err)
			}
		}
		mem = primary().mem()
	case "maxv5":
		if strconv.IntSize != 64 {
			t.Skip("MaxV5Name on a 32-bit host is below the slice limit")
		}
		name := make([]byte, MaxV5Name)
		pages, ok := scratchPagesFor(uint64(len(name)) + v5NameOffset)
		if !ok {
			t.Fatal("scratchPagesFor refused MaxV5Name")
		}
		in, ok := newInstanceWith(pages, osSource{})
		if !ok {
			t.Fatal("the one-off instance could not grow")
		}
		if _, rc := in.v5(NamespaceDNS, name); rc != 0 {
			t.Fatalf("v5 returned %d", rc)
		}
		mem = in.mem()
	default:
		t.Fatalf("unknown shape %q", shape)
	}
	var ru syscall.Rusage
	if err := syscall.Getrusage(syscall.RUSAGE_SELF, &ru); err != nil {
		t.Fatal(err)
	}
	peak := uint64(ru.Maxrss)
	if runtime.GOOS == "linux" {
		peak <<= 10 // KiB on Linux, bytes on macOS
	}
	fmt.Printf("receipt shape=%s logical=%d retained=%d peak_rss=%d\n", shape, len(mem), cap(mem), peak)
}
