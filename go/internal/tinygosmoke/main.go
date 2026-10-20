// Command tinygosmoke is the check CI runs in headless Chrome: this module compiled by TinyGo
// for the browser (`tinygo build -target=wasm`), with the core linked in from
// staticlib/wasm, loaded by index.html beside it through TinyGo's own wasm_exec.js. It
// prints one PASS or FAIL line per check and DONE at the end; the forge's
// hyper-build-wasm.yml fails the run on any FAIL or a missing DONE, which is also what a
// trap part-way through looks like.
//
// The expected core version comes from rust/Cargo.toml at build time
// (`-ldflags "-X main.wantVersion=..."`), so the page proves the archive that was linked is
// the one this commit's core builds, not a stale one. Stock Go builds this too, natively
// under cgo, so `go vet ./...` covers it like any other package; it is internal, so it is
// nothing a consumer can import.
package main

import (
	"errors"
	"fmt"

	hyperuuid "github.com/SkunkWerkx/HyperUuid/go"
	"github.com/google/uuid"
)

// wantVersion is rust/Cargo.toml's version, set by the build's -ldflags -X.
var wantVersion string

func check(ok bool, what string) {
	if ok {
		fmt.Println("PASS", what)
	} else {
		fmt.Println("FAIL", what)
	}
}

func main() {
	version, err := hyperuuid.NativeVersion()
	fmt.Println("NativeVersion", version, "want", wantVersion)
	check(err == nil && version != "" && version == wantVersion, "NativeVersion matches rust/Cargo.toml")

	a, err := hyperuuid.NewV4()
	b, err2 := hyperuuid.NewV4()
	fmt.Println("v4", a, b)
	check(err == nil && err2 == nil && a.Version() == 4 && a.Variant() == uuid.RFC4122 && a != b, "v4")

	v5, err := hyperuuid.NewV5String(hyperuuid.NamespaceDNS, "www.example.com")
	fmt.Println("v5", v5)
	check(err == nil && v5.String() == "2ed6657d-e927-568b-95e1-2665a8aea6a2", "v5 known answer")

	v7, err := hyperuuid.NewV7()
	created, err2 := hyperuuid.V7Timestamp(v7)
	fmt.Println("v7", v7, created)
	check(err == nil && err2 == nil && v7.Version() == 7 && v7.Variant() == uuid.RFC4122 && created.Year() >= 2026, "v7")

	batch, err := hyperuuid.NewV7Batch(1000)
	ordered := err == nil && len(batch) == 1000
	for i := 1; ordered && i < len(batch); i++ {
		ordered = batch[i].Version() == 7 && batch[i].String() > batch[i-1].String()
	}
	check(ordered, "v7 batch of 1000, strictly increasing")

	sql, err := hyperuuid.V7ToSqlOrder(v7)
	back, err2 := hyperuuid.V7FromSqlOrder(sql)
	check(err == nil && err2 == nil && sql[7]>>4 == 8 && back == v7, "v7 SQL-order round trip through version 8")

	// RFC 9562 Appendix A.5's version 6: a version this package neither mints nor converts.
	v6 := uuid.MustParse("1ec9414c-232a-6b00-b3c8-9f6bdeced846")
	// RFC 9562 Appendix A.6's version 7, fixed: a fresh v7's octet 7 carries the SQL
	// layout's 8 one time in 16, and then it is taken for a SQL-ordered value.
	_, err = hyperuuid.V7FromSqlOrder(uuid.MustParse("017f22e2-79b0-7cc3-98c4-dc0c0c07398f"))
	_, err2 = hyperuuid.V7ToSqlOrder(v6)
	_, err3 := hyperuuid.V7FromSqlOrder(hyperuuid.Nil)
	check(errors.Is(err, hyperuuid.ErrWrongVersion) && errors.Is(err2, hyperuuid.ErrWrongVersion) &&
		errors.Is(err3, hyperuuid.ErrWrongVersion), "SQL-order refusals")

	sqlVersion, err := hyperuuid.VersionIn(sql, hyperuuid.LayoutSqlServer)
	sqlRfc, err2 := hyperuuid.IsRfcIn(sql, 7, hyperuuid.LayoutSqlServer)
	check(err == nil && err2 == nil && sqlVersion == 7 && sqlRfc && hyperuuid.Version(v7) == 7 &&
		hyperuuid.Variant(v7) == hyperuuid.VariantRfc9562 && hyperuuid.IsRfc(v7, 7), "version, variant and IsRfc")

	sqlMillis, err := hyperuuid.V7UnixMillisIn(sql, hyperuuid.LayoutSqlServer)
	millis, err2 := hyperuuid.V7UnixMillis(v7)
	check(err == nil && err2 == nil && sqlMillis == millis, "timestamp read in SQL order")

	stamped, err := hyperuuid.GetTimestampIn(sql, hyperuuid.LayoutSqlServer)
	notRfc := v7
	notRfc[8] &^= 0xC0 // the NCS variant: a 7 nibble, but no RFC version and so no timestamp
	_, err2 = hyperuuid.GetTimestamp(notRfc)
	_, err3 = hyperuuid.GetTimestamp(v6)
	check(err == nil && stamped.Equal(created) && errors.Is(err2, hyperuuid.ErrNotTimeBased) &&
		errors.Is(err3, hyperuuid.ErrNotTimeBased), "GetTimestamp reads RFC 9562 v7 only")

	_, err = hyperuuid.NewV7Batch(hyperuuid.MaxV7Batch + 1)
	check(errors.Is(err, hyperuuid.ErrBatchTooLarge), "v7 batch past MaxV7Batch refused")

	fmt.Println("DONE")
}
