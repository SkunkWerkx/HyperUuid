// Command wasm2go regenerates go/internal/core/core.go: the Rust core built for
// wasm32-wasip1 by the rustc pinned in rust-toolchain.toml beside this file, translated to
// one Go source file by the wasm2go pinned in this module's go.mod. It is run by
// `go generate` in go/ and by the regeneration tests, and never ships: this module is a tool
// module, not part of the published binding.
//
// Exit status 2 means a prerequisite is missing (cargo, rustup, the pinned toolchain or its
// wasm32-wasip1 target) and the install line is on stderr; 1 means cargo or wasm2go itself
// failed and their output is on stderr. The tests tell the two apart by the status, never
// by parsing the text.
package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strconv"
	"strings"
)

// defaultStack is the guest's shadow stack in bytes, passed to the linker as
// -zstack-size. rust-lld's default is 1 MiB; an instance must be small enough to pool (the
// binding keeps one primary and a pool of instances for stateless calls), and the core's
// deepest path needs far less. The margin is measured, not proved: regenerating at halving
// sizes and running the whole suite, the receipts (a batch of MaxV7Batch, a v5 of
// MaxV5Name) and the exhaustion package at each, 256 bytes is the smallest size that passes
// everything and 128 is the first that fails, with a bounds panic from the generated code
// (the stack pointer runs below zero and the next access traps), never a wrong UUID
// (2026-10-31, Apple M6, rustc 1.99.1). 65536 is 256x the smallest passing size, against a
// floor of 8x. The flag goes on the command line here, never into rust/.cargo/config.toml,
// so the crate's own wasip1 test suite and Swift's WebAssembly archive keep rust-lld's
// default.
const defaultStack = 65536

const target = "wasm32-wasip1"

var channelLine = regexp.MustCompile(`(?m)^channel = "([^"]+)"`)

func main() {
	os.Exit(run())
}

func run() int {
	_, self, _, ok := runtime.Caller(0)
	if !ok {
		fmt.Fprintln(os.Stderr, "wasm2go: cannot locate this source file")
		return 1
	}
	toolDir := filepath.Dir(self)
	defaultRoot := filepath.Clean(filepath.Join(toolDir, "..", "..", ".."))

	root := flag.String("root", defaultRoot, "repository root (holds rust/ and go/)")
	out := flag.String("o", "", "output file (default <root>/go/internal/core/core.go)")
	stack := flag.Int("stack", defaultStack, "guest stack size in bytes (-zstack-size)")
	flag.Parse()
	if *out == "" {
		*out = filepath.Join(*root, "go", "internal", "core", "core.go")
	}

	pin, err := pinnedToolchain(toolDir)
	if err != nil {
		fmt.Fprintln(os.Stderr, "wasm2go:", err)
		return 1
	}
	install := fmt.Sprintf("rustup toolchain install %s --profile minimal --target %s", pin, target)

	// Prerequisites: exit 2 with the install line.
	for _, tool := range []string{"cargo", "rustup"} {
		if _, err := exec.LookPath(tool); err != nil {
			fmt.Fprintf(os.Stderr, "wasm2go: %s is not on PATH; install Rust, then run\n  %s\n", tool, install)
			return 2
		}
	}
	if msg, err := exec.Command("rustup", "run", pin, "rustc", "-V").CombinedOutput(); err != nil {
		fmt.Fprintf(os.Stderr, "wasm2go: toolchain %s is not installed (%s); run\n  %s\n", pin, strings.TrimSpace(string(msg)), install)
		return 2
	}
	if msg, err := exec.Command("rustup", "target", "list", "--installed", "--toolchain", pin).Output(); err != nil || !strings.Contains(string(msg), target) {
		fmt.Fprintf(os.Stderr, "wasm2go: target %s is not installed for %s; run\n  %s\n", target, pin, install)
		return 2
	}

	// Build the core. `cargo cdylib` is the crate's alias (rust/.cargo/config.toml); the
	// arguments after `--` reach rustc.
	rustDir := filepath.Join(*root, "rust")
	targetDir := filepath.Join(rustDir, "target")
	cargo := exec.Command("cargo", "+"+pin, "cdylib", "--target", target, "--target-dir", targetDir,
		"--", "-C", "link-arg=-zstack-size="+strconv.Itoa(*stack))
	cargo.Dir = rustDir
	if msg, err := cargo.CombinedOutput(); err != nil {
		fmt.Fprintf(os.Stderr, "wasm2go: cargo failed in %s: %v\n%s", rustDir, err, msg)
		return 1
	}
	wasm := filepath.Join(targetDir, target, "release", "hyperuuid.wasm")

	// Translate. `go tool wasm2go` resolves through this module's go.mod, so the version is
	// the pinned one wherever the caller's working directory is.
	if err := os.MkdirAll(filepath.Dir(*out), 0o755); err != nil {
		fmt.Fprintln(os.Stderr, "wasm2go:", err)
		return 1
	}
	translate := exec.Command("go", "tool", "wasm2go", "-pkg", "core", "-o", *out, wasm)
	translate.Dir = toolDir
	if msg, err := translate.CombinedOutput(); err != nil {
		fmt.Fprintf(os.Stderr, "wasm2go: translation failed: %v\n%s", err, msg)
		return 1
	}
	return 0
}

func pinnedToolchain(toolDir string) (string, error) {
	data, err := os.ReadFile(filepath.Join(toolDir, "rust-toolchain.toml"))
	if err != nil {
		return "", fmt.Errorf("reading the toolchain pin: %w", err)
	}
	m := channelLine.FindSubmatch(data)
	if m == nil {
		return "", fmt.Errorf("rust-toolchain.toml has no channel line")
	}
	return string(m[1]), nil
}
