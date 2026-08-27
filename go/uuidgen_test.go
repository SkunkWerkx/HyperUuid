package hyperuuid

import (
	"errors"
	"testing"

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
