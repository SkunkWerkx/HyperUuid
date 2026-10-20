package hyperuuid

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/google/uuid"
)

// ---- The load probes -----------------------------------------------------------------

func TestAvailableAndLoadErrorAgree(t *testing.T) {
	// The core is linked in, so both are constants; this pins that they stay that way.
	if !Available() || LoadError() != nil {
		t.Fatalf("Available() = %v, LoadError() = %v; a linked core is always available", Available(), LoadError())
	}
}

func TestNativeVersionMatchesTheCrate(t *testing.T) {
	// The expectation is the crate's own manifest, walked up to from the test directory, so
	// a release bump never leaves a stale literal here. A device run (Android, through
	// .github/scripts/android_device_test.sh) has no source tree and is handed it.
	if want := os.Getenv("HYPERUUID_CRATE_VERSION"); want != "" {
		got, err := NativeVersion()
		if err != nil {
			t.Fatal(err)
		}
		if got != want {
			t.Fatalf("got %q, HYPERUUID_CRATE_VERSION says %q", got, want)
		}
		return
	}
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		text, readErr := os.ReadFile(filepath.Join(dir, "rust", "Cargo.toml"))
		if readErr == nil {
			for _, line := range strings.Split(string(text), "\n") {
				line = strings.TrimRight(line, "\r")
				if rest, ok := strings.CutPrefix(line, "version = \""); ok {
					want := rest[:strings.IndexByte(rest, '"')]
					got, err := NativeVersion()
					if err != nil {
						t.Fatal(err)
					}
					if got != want {
						t.Fatalf("got %q, rust/Cargo.toml says %q", got, want)
					}
					return
				}
			}
			t.Fatal("no version line in rust/Cargo.toml")
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("rust/Cargo.toml not found above the test directory")
		}
		dir = parent
	}
}

func TestV4HasVersionAndVariantBits(t *testing.T) {
	id, err := NewV4()
	if err != nil {
		t.Fatal(err)
	}
	if id.Version() != 4 {
		t.Errorf("version = %d, want 4", id.Version())
	}
	if id.Variant() != uuid.RFC4122 {
		t.Errorf("variant = %v, want RFC4122", id.Variant())
	}
}

// Proves V7Timestamp isn't just reading back what our own NewV7 wrote — it's a plain RFC
// 9562 bit-layout read, so it recovers the real embedded timestamp from a version 7 UUID
// minted by google/uuid's own native generator too.
func TestV7TimestampExtractsFromGoogleUuidsNativeGenerator(t *testing.T) {
	before := time.Now()
	id, err := uuid.NewV7()
	if err != nil {
		t.Fatal(err)
	}
	after := time.Now()

	got, err := V7Timestamp(id)
	if err != nil {
		t.Fatal(err)
	}
	if got.Before(before.Truncate(time.Millisecond)) || got.After(after) {
		t.Errorf("V7Timestamp(uuid.NewV7()) = %v, want within [%v, %v]", got, before, after)
	}
}

func TestV4IsNonDeterministic(t *testing.T) {
	seen := make(map[uuid.UUID]struct{}, 100)
	for i := 0; i < 100; i++ {
		id, err := NewV4()
		if err != nil {
			t.Fatal(err)
		}
		seen[id] = struct{}{}
	}
	if len(seen) != 100 {
		t.Errorf("got %d distinct UUIDs, want 100", len(seen))
	}
}

// RFC 9562 Appendix A.4 official test vector.
func TestV5MatchesRfcTestVector(t *testing.T) {
	id, err := NewV5String(NamespaceDNS, "www.example.com")
	if err != nil {
		t.Fatal(err)
	}
	want := uuid.MustParse("2ed6657d-e927-568b-95e1-2665a8aea6a2")
	if id != want {
		t.Errorf("got %s, want %s", id, want)
	}
}

// Python's `uuid` standard library documentation test vector.
func TestV5MatchesPythonDocsVector(t *testing.T) {
	id, err := NewV5String(NamespaceDNS, "python.org")
	if err != nil {
		t.Fatal(err)
	}
	want := uuid.MustParse("886313e1-3b8a-5372-9b90-0c9aee199e5d")
	if id != want {
		t.Errorf("got %s, want %s", id, want)
	}
}

func TestV5IsDeterministic(t *testing.T) {
	a, err := NewV5String(NamespaceDNS, "same-name")
	if err != nil {
		t.Fatal(err)
	}
	b, err := NewV5String(NamespaceDNS, "same-name")
	if err != nil {
		t.Fatal(err)
	}
	if a != b {
		t.Errorf("got %s and %s, want equal", a, b)
	}
}

func TestV5DifferentNamespacesDiffer(t *testing.T) {
	dns, err := NewV5String(NamespaceDNS, "test")
	if err != nil {
		t.Fatal(err)
	}
	url, err := NewV5String(NamespaceURL, "test")
	if err != nil {
		t.Fatal(err)
	}
	if dns == url {
		t.Errorf("got equal UUIDs for different namespaces: %s", dns)
	}
}

// The raw-byte door is the primary one — NewV5String is a conversion in front of it — and
// a name is bytes, not text: nothing requires it to be valid UTF-8. google/uuid's NewSHA1 is
// the same RFC 9562 §5.5 construction in pure Go, so it stands in as the oracle.
func TestV5RawBytesAgreeWithTheStringFormAndGoogleUuid(t *testing.T) {
	fromBytes, err := NewV5(NamespaceDNS, []byte("www.example.com"))
	if err != nil {
		t.Fatal(err)
	}
	fromString, err := NewV5String(NamespaceDNS, "www.example.com")
	if err != nil {
		t.Fatal(err)
	}
	if fromBytes != fromString {
		t.Errorf("NewV5 gave %s, NewV5String gave %s", fromBytes, fromString)
	}

	binary := []byte{0x00, 0xff, 0xfe, 0x80, 0x00, 0x7f}
	got, err := NewV5(NamespaceOID, binary)
	if err != nil {
		t.Fatal(err)
	}
	if want := uuid.NewSHA1(NamespaceOID, binary); got != want {
		t.Errorf("got %s, want %s", got, want)
	}
	if got.Version() != 5 || got.Variant() != uuid.RFC4122 {
		t.Errorf("version %d variant %v, want 5 / RFC4122", got.Version(), got.Variant())
	}
}

// An empty name is valid — it hashes the namespace alone — and crosses the ABI as a NULL
// pointer with a zero length, which the core must not dereference. nil, an empty slice and
// the empty string are the same name.
func TestV5EmptyNameIsValid(t *testing.T) {
	want := uuid.NewSHA1(NamespaceDNS, nil)

	fromNil, err := NewV5(NamespaceDNS, nil)
	if err != nil {
		t.Fatal(err)
	}
	fromEmpty, err := NewV5(NamespaceDNS, []byte{})
	if err != nil {
		t.Fatal(err)
	}
	fromString, err := NewV5String(NamespaceDNS, "")
	if err != nil {
		t.Fatal(err)
	}
	for form, got := range map[string]uuid.UUID{"nil": fromNil, "empty slice": fromEmpty, "empty string": fromString} {
		if got != want {
			t.Errorf("%s: got %s, want %s", form, got, want)
		}
	}
}

// RFC 9562 Appendix A.6: 2022-02-22T19:22:22Z = 1645557742000 ms since epoch.
const rfcTestVectorMs uint64 = 1_645_557_742_000

func TestBatchRejectsANegativeCount(t *testing.T) {
	if _, err := NewV7BatchAt(-1, rfcTestVectorMs); !errors.Is(err, ErrNegativeCount) {
		t.Errorf("got %v, want ErrNegativeCount", err)
	}
}

func TestNilAndMax(t *testing.T) {
	if Nil.String() != "00000000-0000-0000-0000-000000000000" {
		t.Errorf("Nil = %s, want all zeros", Nil)
	}
	if Max.String() != "ffffffff-ffff-ffff-ffff-ffffffffffff" {
		t.Errorf("Max = %s, want all ones", Max)
	}
}

func TestV7EmbedsTheTimestamp(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	embeddedMs := uint64(id[0])<<40 | uint64(id[1])<<32 | uint64(id[2])<<24 | uint64(id[3])<<16 | uint64(id[4])<<8 | uint64(id[5])
	if embeddedMs != rfcTestVectorMs {
		t.Errorf("embedded ms = %d, want %d", embeddedMs, rfcTestVectorMs)
	}
}

func TestV7HasVersionAndVariantBits(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	if id.Version() != 7 {
		t.Errorf("version = %d, want 7", id.Version())
	}
	if id.Variant() != uuid.RFC4122 {
		t.Errorf("variant = %v, want RFC4122", id.Variant())
	}
}

func TestV7OverflowTimestampErrors(t *testing.T) {
	_, err := NewV7At(0x0001_0000_0000_0000)
	if !errors.Is(err, ErrTimestampOutOfRange) {
		t.Errorf("got %v, want ErrTimestampOutOfRange", err)
	}
}

func TestV7SameMillisecondBatchIsMonotonicallyOrdered(t *testing.T) {
	ids := make([]uuid.UUID, 100)
	for i := range ids {
		id, err := NewV7At(rfcTestVectorMs)
		if err != nil {
			t.Fatal(err)
		}
		ids[i] = id
	}
	for i := 1; i < len(ids); i++ {
		if ids[i-1].String() > ids[i].String() {
			t.Errorf("ids[%d]=%s > ids[%d]=%s, want non-decreasing", i-1, ids[i-1], i, ids[i])
		}
	}
}

func TestV7CurrentTimestampIsEmbedded(t *testing.T) {
	id, err := NewV7()
	if err != nil {
		t.Fatal(err)
	}
	if id.Version() != 7 {
		t.Errorf("version = %d, want 7", id.Version())
	}
}

func TestV7TimestampRecoversTheExactMillisecond(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	got, err := V7Timestamp(id)
	if err != nil {
		t.Fatal(err)
	}
	want := time.UnixMilli(int64(rfcTestVectorMs)).UTC()
	if !got.Equal(want) {
		t.Errorf("got %v, want %v", got, want)
	}
}

func TestV7TimestampRoundTripsZeroAndTheRfc48BitMax(t *testing.T) {
	zero, err := NewV7At(0)
	if err != nil {
		t.Fatal(err)
	}
	got, err := V7Timestamp(zero)
	if err != nil {
		t.Fatal(err)
	}
	if !got.Equal(time.UnixMilli(0).UTC()) {
		t.Errorf("got %v, want unix epoch", got)
	}

	const maxMs uint64 = 0x0000_FFFF_FFFF_FFFF
	id, err := NewV7At(maxMs)
	if err != nil {
		t.Fatal(err)
	}
	got, err = V7Timestamp(id)
	if err != nil {
		t.Fatal(err)
	}
	if uint64(got.UnixMilli()) != maxMs {
		t.Errorf("got %d ms, want %d", got.UnixMilli(), maxMs)
	}
}

func TestNewV7AtTimeMatchesNewV7AtFromTheEquivalentMillis(t *testing.T) {
	byTime, err := NewV7AtTime(time.UnixMilli(int64(rfcTestVectorMs)).UTC())
	if err != nil {
		t.Fatal(err)
	}
	gotMs, err := V7UnixMillis(byTime)
	if err != nil {
		t.Fatal(err)
	}
	if gotMs != rfcTestVectorMs {
		t.Errorf("got %d ms, want %d", gotMs, rfcTestVectorMs)
	}
}

func TestGetTimestampReturnsErrNotTimeBasedForNonTimeBasedVersions(t *testing.T) {
	id, err := NewV4()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := GetTimestamp(id); !errors.Is(err, ErrNotTimeBased) {
		t.Errorf("got %v, want ErrNotTimeBased", err)
	}
}

func TestGetTimestampMatchesV7Timestamp(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	want, err := V7Timestamp(id)
	if err != nil {
		t.Fatal(err)
	}
	got, err := GetTimestamp(id)
	if err != nil {
		t.Fatal(err)
	}
	if !got.Equal(want) {
		t.Errorf("got %v, want %v", got, want)
	}
}

func TestV7BatchReturnsCountUuidsSortedAndSharingTheTimestamp(t *testing.T) {
	ids, err := NewV7BatchAt(1000, rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	if len(ids) != 1000 {
		t.Fatalf("got %d ids, want 1000", len(ids))
	}
	for i := 1; i < len(ids); i++ {
		if ids[i-1].String() > ids[i].String() {
			t.Errorf("ids[%d]=%s > ids[%d]=%s, want non-decreasing", i-1, ids[i-1], i, ids[i])
		}
	}
	for _, id := range ids {
		got, err := V7Timestamp(id)
		if err != nil {
			t.Fatal(err)
		}
		if want := time.UnixMilli(int64(rfcTestVectorMs)).UTC(); !got.Equal(want) {
			t.Errorf("got %v, want %v", got, want)
		}
	}
}

func TestV7BatchContinuesTheSameCounterSequenceAsIndividualCalls(t *testing.T) {
	before, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	batch, err := NewV7BatchAt(10, rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	after, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}

	ids := append([]uuid.UUID{before}, batch...)
	ids = append(ids, after)
	for i := 1; i < len(ids); i++ {
		if ids[i-1].String() > ids[i].String() {
			t.Errorf("ids[%d]=%s > ids[%d]=%s, want non-decreasing", i-1, ids[i-1], i, ids[i])
		}
	}
}

func TestV7BatchCountZeroReturnsNil(t *testing.T) {
	ids, err := NewV7BatchAt(0, rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	if len(ids) != 0 {
		t.Errorf("got %d ids, want 0", len(ids))
	}
}

func TestV7BatchOverflowTimestampErrors(t *testing.T) {
	_, err := NewV7BatchAt(1, 0x0001_0000_0000_0000)
	if !errors.Is(err, ErrTimestampOutOfRange) {
		t.Errorf("got %v, want ErrTimestampOutOfRange", err)
	}
}

func TestV7ToSqlOrderRoundTripsThroughV7FromSqlOrder(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	sqlOrdered, err := V7ToSqlOrder(id)
	if err != nil {
		t.Fatal(err)
	}
	if sqlOrdered == id {
		t.Fatal("V7ToSqlOrder returned the input unchanged, want the bytes actually converted")
	}
	roundTripped, err := V7FromSqlOrder(sqlOrdered)
	if err != nil {
		t.Fatal(err)
	}
	if roundTripped != id {
		t.Errorf("V7FromSqlOrder(V7ToSqlOrder(id)) = %v, want %v", roundTripped, id)
	}
}

// The conversion writes a version 8: the 8 at octet 7's top nibble and the RFC variant at
// octet 8's top two bits, where SQL Server's text form and a .NET Guid show them.
func TestSqlOrderWritesVersion8WithTheRfcVariant(t *testing.T) {
	for i := 0; i < 256; i++ {
		sqlOrdered, err := V7ToSqlOrder(must(NewV7At(rfcTestVectorMs)))
		if err != nil {
			t.Fatal(err)
		}
		if sqlOrdered[7]&0xF0 != 0x80 {
			t.Fatalf("%x: version nibble at octet 7 = %#x, want 0x80..0x8F", sqlOrdered, sqlOrdered[7])
		}
		if sqlOrdered[8]&0xC0 != 0x80 {
			t.Fatalf("%x: variant bits at octet 8 = %#x, want top two bits 10", sqlOrdered, sqlOrdered[8])
		}
	}
}

// Each direction takes only its own input, and the refusal is the wrong-version error with
// nothing returned. Fixed vectors only: a fresh v7's octet 7 is counter bits, which carry the
// SQL layout's 8 one time in 16, and its counter moves too slowly for draws in a loop to help.
func TestSqlOrderTakesOnlyItsOwnInput(t *testing.T) {
	if got, err := V7FromSqlOrder(rfcV7); !errors.Is(err, ErrWrongVersion) || got != Nil {
		t.Fatalf("%v: V7FromSqlOrder of an RFC-ordered v7 = %v, %v, want ErrWrongVersion", rfcV7, got, err)
	}
	if got, err := V7ToSqlOrder(rfcV7); err != nil || got != rfcV7Sql {
		t.Fatalf("V7ToSqlOrder(%v) = %v, %v, want %v", rfcV7, got, err, rfcV7Sql)
	}
	if got, err := V7ToSqlOrder(rfcV7Sql); !errors.Is(err, ErrWrongVersion) || got != Nil {
		t.Errorf("%v: V7ToSqlOrder of a SQL-ordered value = %v, %v, want ErrWrongVersion", rfcV7Sql, got, err)
	}
	if got, err := V7ToSqlOrder(rfcV6); !errors.Is(err, ErrWrongVersion) || got != Nil {
		t.Errorf("%x: V7ToSqlOrder of a v6 = %x, %v, want ErrWrongVersion", rfcV6, got, err)
	}
}

// The refusal carries the exact message every binding shares.
func TestSqlOrderRefusals(t *testing.T) {
	for name, err := range map[string]error{
		"a v4 to SQL as a v7": second(V7ToSqlOrder(uuid.MustParse("919108f7-52d1-4320-9bac-f847db4148a8"))),
		"a v6 to SQL as a v7": second(V7ToSqlOrder(rfcV6)),
	} {
		const msg = "hyperuuid: the value is not a UUID of the version this conversion takes"
		if !errors.Is(err, ErrWrongVersion) || err.Error() != msg {
			t.Errorf("%s: got %v, want %q", name, err, msg)
		}
	}
}

// There is no timestamp limit: every version 7 converts and comes back, the latest one, at
// 2^48 - 1 ms, included.
func TestEveryV7TimestampConverts(t *testing.T) {
	for _, s := range []string{
		"00000000-0000-7000-8000-000000000000",
		"7fffffff-ffff-7000-8000-000000000000",
		"80000000-0000-7000-8000-000000000000",
		"ffffffff-ffff-7fff-bfff-ffffffffffff",
	} {
		id := uuid.MustParse(s)
		sql, err := V7ToSqlOrder(id)
		if err != nil {
			t.Errorf("%s: V7ToSqlOrder: %v", s, err)
			continue
		}
		if back, err := V7FromSqlOrder(sql); err != nil || back != id {
			t.Errorf("%s: round trip = %x, %v", s, back, err)
		}
		if got, err := V7UnixMillisIn(sql, LayoutSqlServer); err != nil || got != v7UnixMillis(id) {
			t.Errorf("%s: V7UnixMillisIn = %d, %v, want %d", s, got, err, v7UnixMillis(id))
		}
	}
}

// second is the error of a (value, error) pair.
func second[T any](_ T, err error) error { return err }

// sqlGuidCompare replicates System.Data.SqlTypes.SqlGuid.CompareTo's fixed byte significance
// order — the correctness oracle this project's C# test suite checks directly against the
// real type; no Go equivalent exists to test against here, so this stands in for it.
func sqlGuidCompare(a, b [16]byte) int {
	significanceOrder := [16]int{10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3}
	for _, i := range significanceOrder {
		if a[i] != b[i] {
			if a[i] < b[i] {
				return -1
			}
			return 1
		}
	}
	return 0
}

func TestV7ToSqlOrderSortsByCreationOrderUnderSqlGuidComparison(t *testing.T) {
	var ids []uuid.UUID
	for i := uint64(0); i < 200; i++ {
		id, err := NewV7At(rfcTestVectorMs + i)
		if err != nil {
			t.Fatal(err)
		}
		ids = append(ids, id)
	}
	// Same-millisecond run, so the counter (not just the timestamp) has to sort correctly too.
	for i := 0; i < 200; i++ {
		id, err := NewV7At(rfcTestVectorMs + 1_000_000)
		if err != nil {
			t.Fatal(err)
		}
		ids = append(ids, id)
	}

	sqlOrdered := make([][16]byte, len(ids))
	for i, id := range ids {
		sql, err := V7ToSqlOrder(id)
		if err != nil {
			t.Fatal(err)
		}
		sqlOrdered[i] = [16]byte(sql)
	}

	sorted := make([][16]byte, len(sqlOrdered))
	copy(sorted, sqlOrdered)
	sort.Slice(sorted, func(i, j int) bool { return sqlGuidCompare(sorted[i], sorted[j]) < 0 })

	for i := range sqlOrdered {
		if sqlOrdered[i] != sorted[i] {
			t.Fatalf("SQL-ordered bytes do not sort in creation order under SqlGuid comparison at index %d", i)
		}
	}
}

// ---- Destination-buffer fills -------------------------------------------------------

func TestFillV7AtFillsTheCallersSlice(t *testing.T) {
	dst := make([]uuid.UUID, 64)
	if err := FillV7At(dst, rfcTestVectorMs); err != nil {
		t.Fatalf("FillV7At: %v", err)
	}
	for i, id := range dst {
		if id.Version() != 7 {
			t.Fatalf("item %d: version %d, want 7", i, id.Version())
		}
		ms, err := V7UnixMillis(id)
		if err != nil {
			t.Fatalf("item %d: %v", i, err)
		}
		if ms != rfcTestVectorMs {
			t.Fatalf("item %d: timestamp %d, want %d", i, ms, rfcTestVectorMs)
		}
	}
}

func TestFillV7AtIsStrictlyIncreasing(t *testing.T) {
	dst := make([]uuid.UUID, 256)
	if err := FillV7At(dst, rfcTestVectorMs); err != nil {
		t.Fatalf("FillV7At: %v", err)
	}
	for i := 1; i < len(dst); i++ {
		if bytes.Compare(dst[i-1][:], dst[i][:]) >= 0 {
			t.Fatalf("items %d/%d not in creation order: %s then %s", i-1, i, dst[i-1], dst[i])
		}
	}
}

func TestFillEmptyAndNilAreNoOps(t *testing.T) {
	if err := FillV7At(nil, rfcTestVectorMs); err != nil {
		t.Fatalf("nil slice: %v", err)
	}
	if err := FillV7At([]uuid.UUID{}, rfcTestVectorMs); err != nil {
		t.Fatalf("empty slice: %v", err)
	}
	if err := FillV7BytesAt(nil, rfcTestVectorMs); err != nil {
		t.Fatalf("nil bytes: %v", err)
	}
}

func TestFillV7AtRejectsAnOutOfRangeTimestamp(t *testing.T) {
	dst := make([]uuid.UUID, 4)
	err := FillV7At(dst, 1<<48)
	if !errors.Is(err, ErrTimestampOutOfRange) {
		t.Fatalf("got %v, want ErrTimestampOutOfRange", err)
	}
}

// ---- Raw-byte fills -----------------------------------------------------------------

func TestFillV7BytesAtMatchesTheSliceForm(t *testing.T) {
	const count = 16
	raw := make([]byte, count*16)
	if err := FillV7BytesAt(raw, rfcTestVectorMs); err != nil {
		t.Fatalf("FillV7BytesAt: %v", err)
	}
	for i := 0; i < count; i++ {
		var id uuid.UUID
		copy(id[:], raw[i*16:(i+1)*16])
		if id.Version() != 7 {
			t.Fatalf("item %d: version %d, want 7", i, id.Version())
		}
		ms, err := V7UnixMillis(id)
		if err != nil || ms != rfcTestVectorMs {
			t.Fatalf("item %d: ms=%d err=%v", i, ms, err)
		}
	}
}

func TestFillBytesRejectsAPartialUUID(t *testing.T) {
	if err := FillV7BytesAt(make([]byte, 17), rfcTestVectorMs); !errors.Is(err, ErrBufferNotWholeUUIDs) {
		t.Fatalf("got %v, want ErrBufferNotWholeUUIDs", err)
	}
	if err := FillV7Bytes(make([]byte, 15)); !errors.Is(err, ErrBufferNotWholeUUIDs) {
		t.Fatalf("got %v, want ErrBufferNotWholeUUIDs", err)
	}
}

// ---- Raw-byte SQL-order transforms --------------------------------------------------

func TestV7ToSqlOrderBytesAgreesWithTheUUIDForm(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatalf("NewV7At: %v", err)
	}
	want, err := V7ToSqlOrder(id)
	if err != nil {
		t.Fatalf("V7ToSqlOrder: %v", err)
	}
	got := make([]byte, 16)
	copy(got, id[:])
	if err := V7ToSqlOrderBytes(got); err != nil {
		t.Fatalf("V7ToSqlOrderBytes: %v", err)
	}
	if !bytes.Equal(got, want[:]) {
		t.Fatalf("byte form %x != uuid form %x", got, want[:])
	}
}

func TestSqlOrderBytesRoundTrips(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatalf("NewV7At: %v", err)
	}
	b := make([]byte, 16)
	copy(b, id[:])
	original := append([]byte(nil), b...)

	if err := V7ToSqlOrderBytes(b); err != nil {
		t.Fatalf("to: %v", err)
	}
	if bytes.Equal(b, original) {
		t.Fatal("sql order did not change the bytes")
	}
	if err := V7FromSqlOrderBytes(b); err != nil {
		t.Fatalf("from: %v", err)
	}
	if !bytes.Equal(b, original) {
		t.Fatalf("round trip lost data: %x != %x", b, original)
	}
}

func TestSqlOrderBytesLeavesARefusedBufferAsItWas(t *testing.T) {
	b := append([]byte(nil), rfcV6[:]...)
	for name, convert := range map[string]func([]byte) error{
		"V7ToSqlOrderBytes": V7ToSqlOrderBytes, "V7FromSqlOrderBytes": V7FromSqlOrderBytes,
	} {
		if err := convert(b); !errors.Is(err, ErrWrongVersion) || !bytes.Equal(b, rfcV6[:]) {
			t.Errorf("%s = %x, %v, want ErrWrongVersion and the bytes unchanged", name, b, err)
		}
	}
}

func TestSqlOrderBytesRejectsAWrongSizedBuffer(t *testing.T) {
	if err := V7ToSqlOrderBytes(make([]byte, 15)); !errors.Is(err, ErrNotOneUUID) {
		t.Fatalf("got %v, want ErrNotOneUUID", err)
	}
	if err := V7FromSqlOrderBytes(make([]byte, 17)); !errors.Is(err, ErrNotOneUUID) {
		t.Fatalf("got %v, want ErrNotOneUUID", err)
	}
}
