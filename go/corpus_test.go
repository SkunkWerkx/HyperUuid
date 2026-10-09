package hyperuuid

// The conformance corpus (corpus/README.md) replayed through this binding's public API. A
// uuid.UUID is its bytes, so every vector's hex is the value as a caller holds it, in either
// layout: a SQL-ordered value is the bytes V7ToSqlOrder/V6ToSqlOrder return.

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/google/uuid"
)

// corpus decodes corpus/<name>, walking up from the test directory to find it.
func corpus(t *testing.T, name string) []map[string]any {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		// testdata/corpus is the copy CI stages for the iOS simulator: Go's go_ios_exec
		// carries the module's files and testdata directories into the app, and nothing
		// above the module, so the repository's corpus/ is not there to walk up to. A device
		// run (Android) has it pushed beside the test binary, its working directory.
		text, readErr := os.ReadFile(filepath.Join(dir, "corpus", name))
		if readErr != nil {
			text, readErr = os.ReadFile(filepath.Join(dir, "testdata", "corpus", name))
		}
		if readErr == nil {
			var vectors []map[string]any
			if err := json.Unmarshal(text, &vectors); err != nil {
				t.Fatalf("corpus/%s: %v", name, err)
			}
			if len(vectors) == 0 {
				t.Fatalf("corpus/%s is empty", name)
			}
			return vectors
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatalf("corpus/%s not found above the test directory", name)
		}
		dir = parent
	}
}

func hexField(t *testing.T, v map[string]any, key string) []byte {
	t.Helper()
	s, ok := v[key].(string)
	if !ok {
		t.Fatalf("%v: no %q", v, key)
	}
	b, err := hex.DecodeString(s)
	if err != nil {
		t.Fatalf("%v: %q: %v", v, key, err)
	}
	return b
}

func uuidField(t *testing.T, v map[string]any, key string) uuid.UUID {
	t.Helper()
	id, err := uuid.FromBytes(hexField(t, v, key))
	if err != nil {
		t.Fatalf("%v: %q: %v", v, key, err)
	}
	return id
}

func layoutField(t *testing.T, v map[string]any) UuidLayout {
	t.Helper()
	switch v["layout"] {
	case "rfc9562":
		return LayoutRfc9562
	case "sql_server":
		return LayoutSqlServer
	default:
		t.Fatalf("%v: unknown layout", v)
		return LayoutUnspecified
	}
}

func intField(t *testing.T, v map[string]any, key string) int {
	t.Helper()
	n, ok := v[key].(float64)
	if !ok {
		t.Fatalf("%v: no %q", v, key)
	}
	return int(n)
}

func TestCorpusV5(t *testing.T) {
	namespaces := map[string]uuid.UUID{
		"dns": NamespaceDNS, "url": NamespaceURL, "oid": NamespaceOID, "x500": NamespaceX500,
	}
	for _, v := range corpus(t, "v5.json") {
		ns, ok := namespaces[v["namespace"].(string)]
		if !ok {
			t.Fatalf("%v: unknown namespace", v)
		}
		want := uuidField(t, v, "expect")
		got, err := NewV5(ns, hexField(t, v, "name_hex"))
		if err != nil || got != want {
			t.Errorf("%v: NewV5 = %v, %v", v, got, err)
		}
		if name, ok := v["name"].(string); ok {
			got, err := NewV5String(ns, name)
			if err != nil || got != want {
				t.Errorf("%v: NewV5String = %v, %v", v, got, err)
			}
		}
	}
}

func TestCorpusSqlOrder(t *testing.T) {
	for _, v := range corpus(t, "sql_order.json") {
		rfc, sql := uuidField(t, v, "rfc"), uuidField(t, v, "sql")
		toSql, fromSql := V7ToSqlOrder, V7FromSqlOrder
		toSqlBytes, fromSqlBytes := V7ToSqlOrderBytes, V7FromSqlOrderBytes
		switch intField(t, v, "version") {
		case 7:
		case 6:
			toSql, fromSql = V6ToSqlOrder, V6FromSqlOrder
			toSqlBytes, fromSqlBytes = V6ToSqlOrderBytes, V6FromSqlOrderBytes
		default:
			t.Fatalf("%v: unknown version", v)
		}

		if got, err := toSql(rfc); err != nil || got != sql {
			t.Errorf("%v: ToSqlOrder = %x, %v", v, got, err)
		}
		if got, err := fromSql(sql); err != nil || got != rfc {
			t.Errorf("%v: FromSqlOrder = %x, %v", v, got, err)
		}
		raw := hexField(t, v, "rfc")
		if err := toSqlBytes(raw); err != nil || !bytes.Equal(raw, sql[:]) {
			t.Errorf("%v: ToSqlOrderBytes = %x, %v", v, raw, err)
		}
		if err := fromSqlBytes(raw); err != nil || !bytes.Equal(raw, rfc[:]) {
			t.Errorf("%v: FromSqlOrderBytes = %x, %v", v, raw, err)
		}
	}
}

func TestCorpusTimestamp(t *testing.T) {
	for _, v := range corpus(t, "timestamp.json") {
		layout := layoutField(t, v)
		id := uuidField(t, v, "uuid")
		ver := intField(t, v, "version")
		where := fmt.Sprint(v)

		if got, err := VersionIn(id, layout); err != nil || got != ver {
			t.Errorf("%s: VersionIn = %d, %v", where, got, err)
		}

		// time.Time holds every timestamp either version can carry, the v7 at 2^48-1 ms
		// (year 10889) included, so no row is out of reach here.
		millisValue, timeBased := v["unix_millis"].(float64)
		if !timeBased {
			if _, err := GetTimestampIn(id, layout); !errors.Is(err, ErrNotTimeBased) {
				t.Errorf("%s: GetTimestampIn err = %v, want ErrNotTimeBased", where, err)
			}
			if layout == LayoutRfc9562 {
				if _, err := GetTimestamp(id); !errors.Is(err, ErrNotTimeBased) {
					t.Errorf("%s: GetTimestamp err = %v, want ErrNotTimeBased", where, err)
				}
			}
			continue
		}
		millis := uint64(millisValue)
		want := time.UnixMilli(int64(millis)).UTC()

		if got, err := GetTimestampIn(id, layout); err != nil || !got.Equal(want) {
			t.Errorf("%s: GetTimestampIn = %v, %v", where, got, err)
		}
		if layout == LayoutRfc9562 {
			if got, err := GetTimestamp(id); err != nil || !got.Equal(want) {
				t.Errorf("%s: GetTimestamp = %v, %v", where, got, err)
			}
		}

		millisIn, stampIn, millisRfc := V7UnixMillisIn, V7TimestampIn, V7UnixMillis
		if ver == 6 {
			millisIn, stampIn, millisRfc = V6UnixMillisIn, V6TimestampIn, V6UnixMillis
		}
		if got, err := millisIn(id, layout); err != nil || got != millis {
			t.Errorf("%s: UnixMillisIn = %d, %v", where, got, err)
		}
		if got, err := stampIn(id, layout); err != nil || !got.Equal(want) {
			t.Errorf("%s: TimestampIn = %v, %v", where, got, err)
		}
		if layout == LayoutRfc9562 {
			if got, err := millisRfc(id); err != nil || got != millis {
				t.Errorf("%s: UnixMillis = %d, %v", where, got, err)
			}
		}
	}
}

func TestCorpusInspect(t *testing.T) {
	variants := map[string]UuidVariant{
		"ncs": VariantNcs, "rfc9562": VariantRfc9562, "microsoft": VariantMicrosoft, "future": VariantFuture,
	}
	for _, v := range corpus(t, "inspect.json") {
		layout := layoutField(t, v)
		raw := hexField(t, v, "uuid")
		id := uuidField(t, v, "uuid")
		ver := intField(t, v, "version")
		wantRfc, _ := v["is_rfc"].(bool)
		where := fmt.Sprint(v)

		if got, err := VersionIn(id, layout); err != nil || got != ver {
			t.Errorf("%s: VersionIn = %d, %v", where, got, err)
		}
		if got, err := VersionBytes(raw, layout); err != nil || got != ver {
			t.Errorf("%s: VersionBytes = %d, %v", where, got, err)
		}
		if got, err := IsRfcIn(id, ver, layout); err != nil || got != wantRfc {
			t.Errorf("%s: IsRfcIn = %v, %v", where, got, err)
		}
		if got, err := IsRfcBytes(raw, ver, layout); err != nil || got != wantRfc {
			t.Errorf("%s: IsRfcBytes = %v, %v", where, got, err)
		}
		for other := 0; other <= 15; other++ {
			if other == ver {
				continue
			}
			if got, err := IsRfcIn(id, other, layout); err != nil || got {
				t.Errorf("%s: IsRfcIn(%d) = %v, %v", where, other, got, err)
			}
		}

		name, hasVariant := v["variant"].(string)
		if !hasVariant {
			continue
		}
		want, ok := variants[name]
		if !ok {
			t.Fatalf("%s: unknown variant", where)
		}
		if got := Variant(id); got != want {
			t.Errorf("%s: Variant = %v", where, got)
		}
		if got, err := VariantBytes(raw); err != nil || got != want {
			t.Errorf("%s: VariantBytes = %v, %v", where, got, err)
		}
		if got := Version(id); got != ver {
			t.Errorf("%s: Version = %d", where, got)
		}
		if got := IsRfc(id, ver); got != wantRfc {
			t.Errorf("%s: IsRfc = %v", where, got)
		}
	}
}
