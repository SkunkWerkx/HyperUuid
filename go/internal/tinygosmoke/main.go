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

	v6, err := hyperuuid.NewV6()
	fmt.Println("v6", v6)
	check(err == nil && v6.Version() == 6 && v6.Variant() == uuid.RFC4122, "v6")

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

	batch6, err := hyperuuid.NewV6Batch(100)
	check(err == nil && len(batch6) == 100 && batch6[99].Version() == 6, "v6 batch of 100")

	sql, err := hyperuuid.V7ToSqlOrder(v7)
	back, err2 := hyperuuid.V7FromSqlOrder(sql)
	check(err == nil && err2 == nil && sql != v7 && back == v7, "v7 SQL-order round trip")

	sqlVersion, err := hyperuuid.VersionIn(sql, hyperuuid.LayoutSqlServer)
	sqlRfc, err2 := hyperuuid.IsRfcIn(sql, 7, hyperuuid.LayoutSqlServer)
	check(err == nil && err2 == nil && sqlVersion == 7 && sqlRfc && hyperuuid.Version(v7) == 7 &&
		hyperuuid.Variant(v7) == hyperuuid.VariantRfc9562 && hyperuuid.IsRfc(v7, 7), "version, variant and IsRfc")

	sqlMillis, err := hyperuuid.V7UnixMillisIn(sql, hyperuuid.LayoutSqlServer)
	millis, err2 := hyperuuid.V7UnixMillis(v7)
	sql6, err3 := hyperuuid.V6ToSqlOrder(v6)
	sql6Millis, err4 := hyperuuid.V6UnixMillisIn(sql6, hyperuuid.LayoutSqlServer)
	millis6, _ := hyperuuid.V6UnixMillis(v6)
	check(err == nil && err2 == nil && err3 == nil && err4 == nil && sqlMillis == millis && sql6Millis == millis6,
		"timestamps read in SQL order")

	stamped, err := hyperuuid.GetTimestampIn(sql, hyperuuid.LayoutSqlServer)
	notRfc := v7
	notRfc[8] &^= 0xC0 // the NCS variant: a 7 nibble, but no RFC version and so no timestamp
	_, err2 = hyperuuid.GetTimestamp(notRfc)
	check(err == nil && stamped.Equal(created) && errors.Is(err2, hyperuuid.ErrNotTimeBased),
		"GetTimestamp reads RFC 9562 v6/v7 only")

	_, err = hyperuuid.NewV7Batch(hyperuuid.MaxV7Batch + 1)
	check(errors.Is(err, hyperuuid.ErrBatchTooLarge), "v7 batch past MaxV7Batch refused")

	fmt.Println("DONE")
}
