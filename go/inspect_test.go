package hyperuuid

// Version/variant inspection and the layout-aware doors against values minted at run time,
// by this package and by google/uuid; corpus_test.go pins the fixed vectors.

import (
	"bytes"
	"errors"
	"runtime"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/google/uuid"
)

const inspectMs uint64 = 1_645_557_742_000

// must unwraps a generator's result; a generator failing here is a broken core, not a case
// under test.
func must(id uuid.UUID, err error) uuid.UUID {
	if err != nil {
		panic(err)
	}
	return id
}

func TestGoogleAndLibraryValuesReportTheirVersionAndTheRfcVariant(t *testing.T) {
	googleV7, err := uuid.NewV7()
	if err != nil {
		t.Fatal(err)
	}
	ours4, err := NewV4()
	if err != nil {
		t.Fatal(err)
	}
	v5, _ := NewV5String(NamespaceDNS, "x")
	v6 := must(NewV6At(inspectMs))
	v7 := must(NewV7At(inspectMs))
	for _, c := range []struct {
		id      uuid.UUID
		version int
	}{{googleV7, 7}, {uuid.New(), 4}, {ours4, 4}, {v5, 5}, {v6, 6}, {v7, 7}} {
		if got := Version(c.id); got != c.version || got != int(c.id.Version()) {
			t.Errorf("%v: Version = %d, want %d (google/uuid says %d)", c.id, got, c.version, c.id.Version())
		}
		if got := Variant(c.id); got != VariantRfc9562 {
			t.Errorf("%v: Variant = %v", c.id, got)
		}
		if !IsRfc(c.id, c.version) {
			t.Errorf("%v: IsRfc(%d) = false", c.id, c.version)
		}
		other := 7
		if c.version == 7 {
			other = 6
		}
		if IsRfc(c.id, other) {
			t.Errorf("%v: IsRfc(%d) = true", c.id, other)
		}
	}
}

func TestNilAndMaxAreClassifiedAsTheRfcSays(t *testing.T) {
	if Version(Nil) != 0 || Variant(Nil) != VariantNcs || IsRfc(Nil, 0) {
		t.Errorf("Nil: version %d, variant %v, IsRfc %v", Version(Nil), Variant(Nil), IsRfc(Nil, 0))
	}
	if Version(Max) != 15 || Variant(Max) != VariantFuture || IsRfc(Max, 15) {
		t.Errorf("Max: version %d, variant %v, IsRfc %v", Version(Max), Variant(Max), IsRfc(Max, 15))
	}
}

func TestAVersionOutsideTheNibbleNeverMatches(t *testing.T) {
	id := must(NewV7At(inspectMs))
	// 7+256 would read as 7 if it were narrowed to a byte, 7+1<<32 if narrowed to a uint32.
	for _, version := range []int{7 + 256, 7 + 1<<32, -1, 16} {
		if IsRfc(id, version) {
			t.Errorf("IsRfc(%d) = true", version)
		}
		if got, err := IsRfcIn(id, version, LayoutRfc9562); err != nil || got {
			t.Errorf("IsRfcIn(%d) = %v, %v", version, got, err)
		}
		if got, err := IsRfcBytes(id[:], version, LayoutRfc9562); err != nil || got {
			t.Errorf("IsRfcBytes(%d) = %v, %v", version, got, err)
		}
	}
}

// In SQL order a v6's random clock_seq sits where a v7's version nibble does and reads as 7
// one time in 16; enough draws that a confusion would surface.
func TestSqlOrderedV6AndV7AreValidatedAndReadInPlaceWithoutConfusion(t *testing.T) {
	want := time.UnixMilli(int64(inspectMs)).UTC()
	for i := 0; i < 2048; i++ {
		six := must(V6ToSqlOrder(must(NewV6At(inspectMs))))
		seven := must(V7ToSqlOrder(must(NewV7At(inspectMs))))

		if got, err := VersionIn(six, LayoutSqlServer); err != nil || got != 6 {
			t.Fatalf("%x: VersionIn = %d, %v", six, got, err)
		}
		if got, err := VersionIn(seven, LayoutSqlServer); err != nil || got != 7 {
			t.Fatalf("%x: VersionIn = %d, %v", seven, got, err)
		}
		for _, c := range []struct {
			id      uuid.UUID
			version int
			want    bool
		}{{six, 6, true}, {six, 7, false}, {seven, 7, true}, {seven, 6, false}} {
			if got, err := IsRfcIn(c.id, c.version, LayoutSqlServer); err != nil || got != c.want {
				t.Fatalf("%x: IsRfcIn(%d) = %v, %v", c.id, c.version, got, err)
			}
		}
		if got, err := V6UnixMillisIn(six, LayoutSqlServer); err != nil || got != inspectMs {
			t.Fatalf("%x: V6UnixMillisIn = %d, %v", six, got, err)
		}
		if got, err := V7UnixMillisIn(seven, LayoutSqlServer); err != nil || got != inspectMs {
			t.Fatalf("%x: V7UnixMillisIn = %d, %v", seven, got, err)
		}
		for _, id := range []uuid.UUID{six, seven} {
			if got, err := GetTimestampIn(id, LayoutSqlServer); err != nil || !got.Equal(want) {
				t.Fatalf("%x: GetTimestampIn = %v, %v", id, got, err)
			}
		}
		// Read straight from SQL order matches permuting back first.
		back, _ := V7FromSqlOrder(seven)
		if in, _ := V7UnixMillisIn(seven, LayoutSqlServer); in != v7UnixMillis(back) {
			t.Fatalf("%x: in place %d, permuted back %d", seven, in, v7UnixMillis(back))
		}
		// The raw form takes the bytes SQL Server stores, which are the value's own.
		if got, err := VersionBytes(seven[:], LayoutSqlServer); err != nil || got != 7 {
			t.Fatalf("%x: VersionBytes = %d, %v", seven, got, err)
		}
	}
}

func TestNonSqlValuesHaveNoSqlVersion(t *testing.T) {
	// Fixed values, not a fresh v4: an RFC v4's random octet 7 reads as a SQL-ordered v7's
	// version nibble one time in 16, beside octet 8's RFC variant bits, and the core rightly
	// cannot tell those 16 bytes from a SQL-ordered v7. (.NET's mixed-endian Guid puts the v4
	// nibble in octet 7, which is why C#'s version of this test can use Guid.NewGuid.)
	v4 := uuid.MustParse("919108f7-52d1-4320-9bac-f847db4148a8") // RFC 9562 A.3
	v5, _ := NewV5String(NamespaceDNS, "x")
	for _, id := range []uuid.UUID{v4, Nil, Max, v5} {
		if got, err := VersionIn(id, LayoutSqlServer); err != nil || got != 0 {
			t.Errorf("%v: VersionIn = %d, %v", id, got, err)
		}
		if _, err := GetTimestampIn(id, LayoutSqlServer); !errors.Is(err, ErrNotTimeBased) {
			t.Errorf("%v: GetTimestampIn err = %v, want ErrNotTimeBased", id, err)
		}
	}
}

func TestAnUnspecifiedOrUndefinedLayoutIsRefused(t *testing.T) {
	id := must(NewV7At(inspectMs))
	raw := id[:]
	for _, layout := range []UuidLayout{LayoutUnspecified, UuidLayout(3)} {
		_, e1 := VersionIn(id, layout)
		_, e2 := IsRfcIn(id, 7, layout)
		_, e3 := V7UnixMillisIn(id, layout)
		_, e4 := V6UnixMillisIn(id, layout)
		_, e5 := V7TimestampIn(id, layout)
		_, e6 := V6TimestampIn(id, layout)
		_, e7 := GetTimestampIn(id, layout)
		_, e8 := VersionBytes(raw, layout)
		_, e9 := IsRfcBytes(raw, 7, layout)
		// The layout is checked before the buffer, as C#'s doors do.
		_, e10 := VersionBytes(raw[:15], layout)
		for i, err := range []error{e1, e2, e3, e4, e5, e6, e7, e8, e9, e10} {
			if !errors.Is(err, ErrInvalidLayout) {
				t.Errorf("%v: door %d err = %v, want ErrInvalidLayout", layout, i+1, err)
			}
		}
	}
	if _, err := VersionIn(id, UuidLayout(3)); err == nil || !strings.Contains(err.Error(), "UuidLayout(3)") {
		t.Errorf("the error names the layout it was given: %v", err)
	}
}

func TestTheRawByteFormsRequireExactly16Bytes(t *testing.T) {
	if _, err := VersionBytes(make([]byte, 15), LayoutRfc9562); !errors.Is(err, ErrNotOneUUID) {
		t.Errorf("VersionBytes: %v", err)
	}
	if _, err := VariantBytes(make([]byte, 17)); !errors.Is(err, ErrNotOneUUID) {
		t.Errorf("VariantBytes: %v", err)
	}
	if _, err := IsRfcBytes(nil, 7, LayoutRfc9562); !errors.Is(err, ErrNotOneUUID) {
		t.Errorf("IsRfcBytes: %v", err)
	}
}

func TestLayoutAndVariantNameThemselves(t *testing.T) {
	for want, got := range map[string]string{
		"LayoutRfc9562": LayoutRfc9562.String(), "LayoutSqlServer": LayoutSqlServer.String(),
		"LayoutUnspecified": LayoutUnspecified.String(), "UuidLayout(9)": UuidLayout(9).String(),
		"VariantNcs": VariantNcs.String(), "VariantRfc9562": VariantRfc9562.String(),
		"VariantMicrosoft": VariantMicrosoft.String(), "VariantFuture": VariantFuture.String(),
		"VariantUnspecified": VariantUnspecified.String(), "UuidVariant(9)": UuidVariant(9).String(),
	} {
		if got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	}
}

// The core's batch codes 3 and 4 are argument errors, not a failed random source.
func TestBatchCodesMapToTheirOwnErrors(t *testing.T) {
	for code, want := range map[int32]error{
		1: ErrRandomSource, 2: ErrTimestampOutOfRange, 3: ErrBatchUnaddressable, 4: ErrBatchTooLarge,
	} {
		err := errBatch("uuid_new_v7_batch", code)
		if !errors.Is(err, want) {
			t.Errorf("code %d: %v, want %v", code, err, want)
		}
		if code > 2 && errors.Is(err, ErrRandomSource) {
			t.Errorf("code %d reports a random source failure: %v", code, err)
		}
	}
}

// The v7 batch limit: the counter space, enforced before anything is allocated or written.
func TestAV7BatchPastTheCounterSpaceIsRefusedOnEveryDoor(t *testing.T) {
	const tooMany = MaxV7Batch + 1
	if MaxV7Batch != 67_108_864 {
		t.Fatalf("MaxV7Batch = %d", MaxV7Batch)
	}
	limit := strconv.Itoa(MaxV7Batch)

	// Real destinations, which the refusal never touches: memory this large comes straight
	// from the OS already zeroed, so nothing is written to it and it costs address space, not
	// resident memory. (A slice claiming a length nothing allocated, C#'s trick, trips
	// checkptr under -race.)
	ids := make([]uuid.UUID, tooMany)
	rawBytes := make([]byte, tooMany*16)
	for name, door := range map[string]func() error{
		"FillV7At":      func() error { return FillV7At(ids, inspectMs) },
		"FillV7":        func() error { return FillV7(ids) },
		"FillV7BytesAt": func() error { return FillV7BytesAt(rawBytes, inspectMs) },
		"FillV7Bytes":   func() error { return FillV7Bytes(rawBytes) },
	} {
		err := door()
		if !errors.Is(err, ErrBatchTooLarge) || !strings.Contains(err.Error(), limit) {
			t.Errorf("%s: %v, want ErrBatchTooLarge naming %s", name, err, limit)
		}
	}
	if ids[0] != Nil || ids[tooMany-1] != Nil || !bytes.Equal(rawBytes[:64], make([]byte, 64)) {
		t.Errorf("a refused fill wrote %x / %x", ids[0], rawBytes[:64])
	}
	runtime.KeepAlive(ids)
	runtime.KeepAlive(rawBytes)

	var before, after runtime.MemStats
	runtime.ReadMemStats(&before)
	for name, door := range map[string]func() ([]uuid.UUID, error){
		"NewV7BatchAt": func() ([]uuid.UUID, error) { return NewV7BatchAt(tooMany, inspectMs) },
		"NewV7Batch":   func() ([]uuid.UUID, error) { return NewV7Batch(tooMany) },
	} {
		got, err := door()
		if got != nil || !errors.Is(err, ErrBatchTooLarge) || !strings.Contains(err.Error(), limit) {
			t.Errorf("%s: %d UUIDs, %v, want ErrBatchTooLarge naming %s", name, len(got), err, limit)
		}
	}
	runtime.ReadMemStats(&after)
	if grew := after.TotalAlloc - before.TotalAlloc; grew > 1<<20 {
		t.Errorf("the refused batches allocated %d bytes", grew)
	}
}

// Exactly the counter space: 1 GiB, in strictly increasing order end to end, and at most a
// millisecond past the supplied timestamp (the roll-forward over the wrap).
func TestAV7BatchOfExactlyTheCounterSpaceIsStrictlyIncreasing(t *testing.T) {
	if testing.Short() {
		t.Skip("allocates 1 GiB; skipped under -short")
	}
	ids, err := NewV7BatchAt(MaxV7Batch, inspectMs)
	if err != nil {
		t.Fatal(err)
	}
	for i := 1; i < len(ids); i++ {
		if bytes.Compare(ids[i][:], ids[i-1][:]) <= 0 {
			t.Fatalf("item %d (%v) does not follow item %d (%v)", i, ids[i], i-1, ids[i-1])
		}
	}
	if last := v7UnixMillis(ids[len(ids)-1]); last != inspectMs && last != inspectMs+1 {
		t.Fatalf("last item stamped %d, want %d or %d", last, inspectMs, inspectMs+1)
	}
}
