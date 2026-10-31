//go:build !tinygo && !js && !wasip1

package hyperuuid

// The regeneration tests prove that internal/core/core.go is what the pinned toolchain
// produces from rust/ today, and that a crate version bump reaches the generated core. They
// are host-only: they need os/exec (stock Go's js and wasip1 ports return ENOSYS from
// StartProcess), a Go and Rust toolchain on PATH, and the Rust tree beside go/ (absent from a
// copied package directory on a mobile runner). Every missing prerequisite is a skip, or a
// failure under HYPERUUID_CHECK_CORE=1, which CI's lint-go sets; anything that goes wrong
// after the prerequisites are established is a defect and fails regardless.

import (
	"bytes"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

// errPrerequisite marks a missing tool, tree or toolchain, as opposed to a defect.
var errPrerequisite = errors.New("regeneration prerequisite missing")

const toolModule = "internal/wasm2go"

func TestCoreIsCurrent(t *testing.T) {
	requireRegeneration(t, "..") // skips on a missing prerequisite; fatal with HYPERUUID_CHECK_CORE=1
	out := filepath.Join(t.TempDir(), "core.go")
	// Builds the generator to a temp executable and runs it with `-o out`; exit 2 is errPrerequisite.
	if err := regenerate(t, "", out); err != nil {
		t.Fatalf("regeneration failed: %v", err) // a defect: the prerequisites were just established
	}
	want := mustRead(t, out)
	got := mustRead(t, "internal/core/core.go")
	if !bytes.Equal(got, want) {
		t.Fatal("internal/core/core.go is stale: run `go generate` in go/")
	}
}

func TestRegenerationCarriesTheCrateVersion(t *testing.T) {
	// A copy of rust/ (without target/) with its version set to 9.8.7, generated into a temp
	// file: the generated hyperuuid_version returns 9<<16|8<<8|7 = 591879 as a literal, and
	// the committed core does not contain it. This is the prepare-release step's behaviour.
	requireRegeneration(t, "..") // before the copy: nothing is copied on a host that cannot regenerate
	root := copyRustTree(t, "9.8.7")
	out := filepath.Join(t.TempDir(), "core.go")
	if err := regenerate(t, root, out); err != nil {
		t.Fatalf("regeneration failed: %v", err)
	}
	if !bytes.Contains(mustRead(t, out), []byte("591879")) {
		t.Fatal("bumped version not in the generated core")
	}
	if bytes.Contains(mustRead(t, "internal/core/core.go"), []byte("591879")) {
		t.Fatal("control failed")
	}
}

// requireRegeneration establishes every prerequisite of regeneration or ends the test: a
// skip when one is missing, a failure under HYPERUUID_CHECK_CORE=1.
func requireRegeneration(t *testing.T, root string) {
	t.Helper()
	err := regenerationPrereqs(root)
	if err == nil {
		return
	}
	if errors.Is(err, errPrerequisite) && os.Getenv("HYPERUUID_CHECK_CORE") == "" {
		t.Skipf("%v; HYPERUUID_CHECK_CORE=1 makes this fatal", err)
	}
	t.Fatalf("regeneration failed: %v", err)
}

// regenerationPrereqs checks, before anything is built or copied, that go, cargo and rustup
// are on PATH, that <root>/rust/Cargo.toml exists, and that the pinned toolchain and its
// wasm32-wasip1 target are installed. Each miss wraps errPrerequisite and names the piece.
func regenerationPrereqs(root string) error {
	for _, tool := range []string{"go", "cargo", "rustup"} {
		if _, err := exec.LookPath(tool); err != nil {
			return fmt.Errorf("%w: %s not on PATH", errPrerequisite, tool)
		}
	}
	manifest := filepath.Join(root, "rust", "Cargo.toml")
	if _, err := os.Stat(manifest); err != nil {
		return fmt.Errorf("%w: %s (%v)", errPrerequisite, filepath.ToSlash(filepath.Join("rust", "Cargo.toml")), err)
	}
	pin, err := pinnedToolchain()
	if err != nil {
		return err
	}
	install := fmt.Sprintf("rustup toolchain install %s --profile minimal --target wasm32-wasip1", pin)
	if out, err := exec.Command("rustup", "run", pin, "rustc", "-V").CombinedOutput(); err != nil {
		return fmt.Errorf("%w: toolchain %s (%v: %s); run `%s`", errPrerequisite, pin, err, strings.TrimSpace(string(out)), install)
	}
	out, err := exec.Command("rustup", "target", "list", "--installed", "--toolchain", pin).Output()
	if err != nil || !strings.Contains(string(out), "wasm32-wasip1") {
		return fmt.Errorf("%w: target wasm32-wasip1 for %s; run `%s`", errPrerequisite, pin, install)
	}
	return nil
}

var channelLine = regexp.MustCompile(`(?m)^channel = "([^"]+)"`)

// pinnedToolchain reads the channel from the tool module's rust-toolchain.toml. A missing or
// malformed file is a defect of this tree, not a prerequisite.
func pinnedToolchain() (string, error) {
	data, err := os.ReadFile(filepath.Join(toolModule, "rust-toolchain.toml"))
	if err != nil {
		return "", fmt.Errorf("reading the toolchain pin: %w", err)
	}
	m := channelLine.FindSubmatch(data)
	if m == nil {
		return "", fmt.Errorf("%s/rust-toolchain.toml has no channel line", toolModule)
	}
	return string(m[1]), nil
}

// regenerate builds the generator into a temp executable and runs it, so the exit status is
// the generator's own (`go run` would report every child failure as exit 1). Exit 2 maps to
// errPrerequisite; any other non-zero exit is an error carrying the generator's stderr.
func regenerate(t *testing.T, root, out string) error {
	t.Helper()
	gen := filepath.Join(t.TempDir(), "wasm2go-gen")
	build := exec.Command("go", "-C", toolModule, "build", "-o", gen, ".")
	if msg, err := build.CombinedOutput(); err != nil {
		return fmt.Errorf("building the generator: %v\n%s", err, msg)
	}
	args := []string{"-o", out}
	if root != "" {
		args = append(args, "-root", root)
	}
	var stderr bytes.Buffer
	cmd := exec.Command(gen, args...)
	cmd.Stderr = &stderr
	cmd.Stdout = &stderr
	err := cmd.Run()
	if err == nil {
		return nil
	}
	var exit *exec.ExitError
	if errors.As(err, &exit) && exit.ExitCode() == 2 {
		return fmt.Errorf("%w: %s", errPrerequisite, strings.TrimSpace(stderr.String()))
	}
	return fmt.Errorf("generator: %v\n%s", err, stderr.String())
}

func mustRead(t *testing.T, path string) []byte {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return data
}

var versionLine = regexp.MustCompile(`(?m)^version = "[^"]*"`)

// copyRustTree copies ../rust minus target/ into <tmp>/rust, sets the crate version in its
// Cargo.toml, and returns the temp root (the generator's -root), which holds rust/ as the
// repository does.
func copyRustTree(t *testing.T, version string) string {
	t.Helper()
	root := t.TempDir()
	src := filepath.Join("..", "rust")
	dst := filepath.Join(root, "rust")
	t.Logf("copying %s to %s at version %s", src, dst, version)
	err := filepath.WalkDir(src, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		if d.IsDir() {
			if rel == "target" {
				return filepath.SkipDir
			}
			return os.MkdirAll(filepath.Join(dst, rel), 0o755)
		}
		if !d.Type().IsRegular() {
			return nil
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		if rel == "Cargo.toml" {
			if !versionLine.Match(data) {
				return fmt.Errorf("%s has no version line", path)
			}
			replaced := false
			data = versionLine.ReplaceAllFunc(data, func(b []byte) []byte {
				if replaced {
					return b
				}
				replaced = true
				return []byte(fmt.Sprintf("version = %q", version))
			})
		}
		info, err := d.Info()
		if err != nil {
			return err
		}
		return os.WriteFile(filepath.Join(dst, rel), data, info.Mode().Perm())
	})
	if err != nil {
		t.Fatalf("copying the Rust tree: %v", err)
	}
	return root
}
