//go:build cgo && (darwin || linux) && !hyperuuid_wasm

package hyperuuid

// The README's zero-allocation claims, held as tests rather than as a benchmark somebody
// has to remember to read. cgo backend only (the build tag is backend_cgo.go's own): the
// by-value shims are what make these zero, and neither purego's trampoline nor
// wasmtime-go's argument boxing can make the same promise.

import (
	"testing"

	"github.com/google/uuid"
)

func assertAllocs(t *testing.T, name string, want float64, f func()) {
	t.Helper()
	if got := testing.AllocsPerRun(100, f); got != want {
		t.Errorf("%s: %v allocs per call, want %v", name, got, want)
	}
}

func TestSingleUuidDoorsDoNotAllocate(t *testing.T) {
	assertAllocs(t, "NewV4", 0, func() {
		if _, err := NewV4(); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "NewV6At", 0, func() {
		if _, err := NewV6At(rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "NewV7At", 0, func() {
		if _, err := NewV7At(rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
	// The name is the caller's own slice, already allocated: nothing is added per call.
	name := []byte("www.example.com")
	assertAllocs(t, "NewV5", 0, func() {
		if _, err := NewV5(NamespaceDNS, name); err != nil {
			t.Fatal(err)
		}
	})
}

func TestExtractionAndSqlOrderDoNotAllocate(t *testing.T) {
	v6, err := NewV6At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	v7, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	assertAllocs(t, "V6Timestamp", 0, func() {
		if _, err := V6Timestamp(v6); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "V7Timestamp", 0, func() {
		if _, err := V7Timestamp(v7); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "GetTimestamp", 0, func() {
		if _, err := GetTimestamp(v7); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "V6ToSqlOrder/V6FromSqlOrder", 0, func() {
		sql, err := V6ToSqlOrder(v6)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := V6FromSqlOrder(sql); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "V7ToSqlOrder/V7FromSqlOrder", 0, func() {
		sql, err := V7ToSqlOrder(v7)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := V7FromSqlOrder(sql); err != nil {
			t.Fatal(err)
		}
	})
	raw := make([]byte, 16)
	copy(raw, v7[:])
	assertAllocs(t, "V7ToSqlOrderBytes/V7FromSqlOrderBytes", 0, func() {
		if err := V7ToSqlOrderBytes(raw); err != nil {
			t.Fatal(err)
		}
		if err := V7FromSqlOrderBytes(raw); err != nil {
			t.Fatal(err)
		}
	})
}

func TestFillsDoNotAllocate(t *testing.T) {
	ids := make([]uuid.UUID, 1000)
	assertAllocs(t, "FillV6At", 0, func() {
		if err := FillV6At(ids, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "FillV7At", 0, func() {
		if err := FillV7At(ids, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
	raw := make([]byte, 1000*16)
	assertAllocs(t, "FillV6BytesAt", 0, func() {
		if err := FillV6BytesAt(raw, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "FillV7BytesAt", 0, func() {
		if err := FillV7BytesAt(raw, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
}

// The slice-returning batch is exactly the one allocation it has to be — the result.
func TestBatchAllocatesOnlyItsResult(t *testing.T) {
	assertAllocs(t, "NewV6BatchAt", 1, func() {
		if _, err := NewV6BatchAt(1000, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "NewV7BatchAt", 1, func() {
		if _, err := NewV7BatchAt(1000, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
}
