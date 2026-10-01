package hyperuuid

import (
	"strings"
	"testing"
)

func TestTargetForMapsEverySupportedPlatform(t *testing.T) {
	for _, c := range []struct {
		goos, goarch string
		musl         bool
		rid, libName string
	}{
		{"linux", "amd64", false, "linux-x64", "libhyperuuid.so"},
		{"linux", "arm64", false, "linux-arm64", "libhyperuuid.so"},
		{"linux", "amd64", true, "linux-musl-x64", "libhyperuuid.so"},
		{"linux", "arm64", true, "linux-musl-arm64", "libhyperuuid.so"},
		{"darwin", "amd64", false, "osx-x64", "libhyperuuid.dylib"},
		{"darwin", "arm64", false, "osx-arm64", "libhyperuuid.dylib"},
		{"windows", "amd64", false, "win-x64", "hyperuuid.dll"},
		{"windows", "arm64", false, "win-arm64", "hyperuuid.dll"},
	} {
		got, err := targetFor(c.goos, c.goarch, c.musl)
		if err != nil {
			t.Errorf("%s/%s musl=%v: %v", c.goos, c.goarch, c.musl, err)
			continue
		}
		if got.rid != c.rid || got.libName != c.libName {
			t.Errorf("%s/%s musl=%v: got %s/%s, want %s/%s",
				c.goos, c.goarch, c.musl, got.rid, got.libName, c.rid, c.libName)
		}
	}
}

// An architecture or OS this module ships no build for is named as unsupported, never
// quietly handed the x64 library.
func TestTargetForRejectsWhatItDoesNotShip(t *testing.T) {
	for _, c := range []struct{ goos, goarch string }{
		{"linux", "386"}, {"linux", "riscv64"}, {"linux", "arm"}, {"windows", "386"},
		{"freebsd", "amd64"}, {"android", "arm64"}, {"ios", "arm64"},
	} {
		got, err := targetFor(c.goos, c.goarch, false)
		if err == nil {
			t.Errorf("%s/%s: got %s, want an error", c.goos, c.goarch, got.rid)
			continue
		}
		if want := "unsupported platform " + c.goos + "/" + c.goarch; !strings.Contains(err.Error(), want) {
			t.Errorf("%s/%s: got %q, want it to say %q", c.goos, c.goarch, err, want)
		}
	}
}

func TestMapsMentionMusl(t *testing.T) {
	const glibc = `55d0c8a00000-55d0c8a01000 r--p 00000000 08:01 1234 /usr/bin/app
7f1e4c000000-7f1e4c028000 r--p 00000000 08:01 5678 /usr/lib/x86_64-linux-gnu/libc.so.6
7f1e4c400000-7f1e4c402000 r--p 00000000 08:01 9012 /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2
`
	const muslLoader = `55d0c8a00000-55d0c8a01000 r--p 00000000 08:01 1234 /usr/bin/app
7f1e4c000000-7f1e4c014000 r--p 00000000 08:01 5678 /lib/ld-musl-x86_64.so.1
`
	const muslLibc = `aaaac8a00000-aaaac8a01000 r--p 00000000 08:01 1234 /usr/bin/app
ffff4c000000-ffff4c014000 r--p 00000000 08:01 5678 /lib/libc.musl-aarch64.so.1
`
	if mapsMentionMusl([]byte(glibc)) {
		t.Error("a glibc process was read as musl")
	}
	if !mapsMentionMusl([]byte(muslLoader)) {
		t.Error("ld-musl-* was not read as musl")
	}
	if !mapsMentionMusl([]byte(muslLibc)) {
		t.Error("libc.musl-* was not read as musl")
	}
	if mapsMentionMusl(nil) {
		t.Error("an empty maps file was read as musl")
	}
}
