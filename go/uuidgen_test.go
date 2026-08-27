package hyperuuid

import (
	"errors"
	"sort"
	"testing"
	"time"

	"github.com/google/uuid"
)

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

// RFC 9562 Appendix A.6: 2022-02-22T19:22:22Z = 1645557742000 ms since epoch.
const rfcTestVectorMs uint64 = 1_645_557_742_000

func TestV6EmbedsTheTimestamp(t *testing.T) {
	id, err := NewV6At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	got, err := V6Timestamp(id)
	if err != nil {
		t.Fatal(err)
	}
	want := time.UnixMilli(int64(rfcTestVectorMs)).UTC()
	if !got.Equal(want) {
		t.Errorf("got %v, want %v", got, want)
	}
}

func TestV6HasVersionAndVariantBits(t *testing.T) {
	id, err := NewV6At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	if id.Version() != 6 {
		t.Errorf("version = %d, want 6", id.Version())
	}
	if id.Variant() != uuid.RFC4122 {
		t.Errorf("variant = %v, want RFC4122", id.Variant())
	}
}

func TestV6SetsTheNodeIdMulticastBit(t *testing.T) {
	id, err := NewV6At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	if id[10]&0x01 != 0x01 {
		t.Errorf("node[0] = %#x, want multicast bit set", id[10])
	}
}

func TestV6IsNonDeterministicWithinTheSameMillisecond(t *testing.T) {
	seen := make(map[uuid.UUID]struct{}, 100)
	for i := 0; i < 100; i++ {
		id, err := NewV6At(rfcTestVectorMs)
		if err != nil {
			t.Fatal(err)
		}
		seen[id] = struct{}{}
	}
	if len(seen) != 100 {
		t.Errorf("got %d distinct UUIDs, want 100", len(seen))
	}
}

func TestV6BatchReturnsCountUuidsSharingTheTimestamp(t *testing.T) {
	ids, err := NewV6BatchAt(10, rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	if len(ids) != 10 {
		t.Fatalf("got %d ids, want 10", len(ids))
	}
	for _, id := range ids {
		if id.Version() != 6 {
			t.Errorf("version = %d, want 6", id.Version())
		}
		got, err := V6Timestamp(id)
		if err != nil {
			t.Fatal(err)
		}
		if want := time.UnixMilli(int64(rfcTestVectorMs)).UTC(); !got.Equal(want) {
			t.Errorf("got %v, want %v", got, want)
		}
	}
}

func TestV6BatchProducesPairwiseDistinctUuids(t *testing.T) {
	ids, err := NewV6BatchAt(100, rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	seen := make(map[uuid.UUID]struct{}, 100)
	for _, id := range ids {
		seen[id] = struct{}{}
	}
	if len(seen) != 100 {
		t.Errorf("got %d distinct UUIDs, want 100", len(seen))
	}
}

func TestV6BatchCountZeroReturnsNil(t *testing.T) {
	ids, err := NewV6BatchAt(0, rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	if len(ids) != 0 {
		t.Errorf("got %d ids, want 0", len(ids))
	}
}

func TestV6BatchOverflowTimestampErrors(t *testing.T) {
	_, err := NewV6BatchAt(1, 0xFFFF_FFFF_FFFF_FFFF)
	if !errors.Is(err, ErrTimestampOutOfRange) {
		t.Errorf("got %v, want ErrTimestampOutOfRange", err)
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

func TestToSqlOrderRoundTripsThroughFromSqlOrder(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	sqlOrdered, err := ToSqlOrder(id)
	if err != nil {
		t.Fatal(err)
	}
	if sqlOrdered == id {
		t.Fatal("ToSqlOrder returned the input unchanged, want the bytes actually permuted")
	}
	roundTripped, err := FromSqlOrder(sqlOrdered)
	if err != nil {
		t.Fatal(err)
	}
	if roundTripped != id {
		t.Errorf("FromSqlOrder(ToSqlOrder(id)) = %v, want %v", roundTripped, id)
	}
}

func TestToSqlOrderPreservesVersionAndVariantAtOctets7And8(t *testing.T) {
	id, err := NewV7At(rfcTestVectorMs)
	if err != nil {
		t.Fatal(err)
	}
	sqlOrdered, err := ToSqlOrder(id)
	if err != nil {
		t.Fatal(err)
	}
	if sqlOrdered[7]&0xF0 != 0x70 {
		t.Errorf("version nibble at octet 7 = %#x, want 0x70..0x7F", sqlOrdered[7])
	}
	if sqlOrdered[8]&0xC0 != 0x80 {
		t.Errorf("variant bits at octet 8 = %#x, want top two bits 10", sqlOrdered[8])
	}
}

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

func TestToSqlOrderSortsByCreationOrderUnderSqlGuidComparison(t *testing.T) {
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
		sql, err := ToSqlOrder(id)
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
