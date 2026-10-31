package hyperuuid

// The README's zero-allocation claims, held as tests rather than as a benchmark somebody
// has to remember to read. Every call copies its arguments into an instance's scratch region
// in the translated core's memory and its result out (core.go), so nothing escapes to the
// heap; a batch allocates only the caller's slice.

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
	v7, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
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
	v6raw := append([]byte(nil), rfcV6[:]...)
	// A refusal returns a prebuilt sentinel, so it allocates nothing either. RFC 9562 A.6's
	// v7, fixed: a fresh v7's octet 7 carries the SQL layout's 8 one time in 16.
	assertAllocs(t, "refused V7FromSqlOrder/V7ToSqlOrderBytes", 0, func() {
		if _, err := V7FromSqlOrder(rfcV7); err != ErrWrongVersion {
			t.Fatal(err)
		}
		if err := V7ToSqlOrderBytes(v6raw); err != ErrWrongVersion {
			t.Fatal(err)
		}
	})
}

func TestFillsDoNotAllocate(t *testing.T) {
	ids := make([]uuid.UUID, 1000)
	assertAllocs(t, "FillV7At", 0, func() {
		if err := FillV7At(ids, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
	raw := make([]byte, 1000*16)
	assertAllocs(t, "FillV7BytesAt", 0, func() {
		if err := FillV7BytesAt(raw, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
}

// The slice-returning batch is exactly the one allocation it has to be — the result.
func TestBatchAllocatesOnlyItsResult(t *testing.T) {
	assertAllocs(t, "NewV7BatchAt", 1, func() {
		if _, err := NewV7BatchAt(1000, rfcTestVectorMs); err != nil {
			t.Fatal(err)
		}
	})
}

func TestInspectionDoesNotAllocate(t *testing.T) {
	v7, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	sql, err := V7ToSqlOrder(v7)
	if err != nil {
		t.Fatal(err)
	}
	raw := make([]byte, 16)
	copy(raw, sql[:])
	assertAllocs(t, "Version/Variant/IsRfc", 0, func() {
		if Version(v7) != 7 || Variant(v7) != VariantRfc9562 || !IsRfc(v7, 7) {
			t.Fatal("not an RFC v7")
		}
	})
	assertAllocs(t, "VersionIn/IsRfcIn", 0, func() {
		if v, err := VersionIn(sql, LayoutSqlServer); err != nil || v != 7 {
			t.Fatal(v, err)
		}
		if ok, err := IsRfcIn(sql, 7, LayoutSqlServer); err != nil || !ok {
			t.Fatal(ok, err)
		}
	})
	assertAllocs(t, "VersionBytes/VariantBytes/IsRfcBytes", 0, func() {
		if v, err := VersionBytes(raw, LayoutSqlServer); err != nil || v != 7 {
			t.Fatal(v, err)
		}
		if ok, err := IsRfcBytes(raw, 7, LayoutSqlServer); err != nil || !ok {
			t.Fatal(ok, err)
		}
		if _, err := VariantBytes(v7[:]); err != nil {
			t.Fatal(err)
		}
	})
	assertAllocs(t, "V7TimestampIn/GetTimestampIn", 0, func() {
		if _, err := V7TimestampIn(sql, LayoutSqlServer); err != nil {
			t.Fatal(err)
		}
		if _, err := GetTimestampIn(sql, LayoutSqlServer); err != nil {
			t.Fatal(err)
		}
	})
	// A value that isn't time-based returns the sentinel itself, not a wrapped copy.
	assertAllocs(t, "GetTimestamp of a non-time-based UUID", 0, func() {
		if _, err := GetTimestamp(Nil); err != ErrNotTimeBased {
			t.Fatal(err)
		}
	})
}
