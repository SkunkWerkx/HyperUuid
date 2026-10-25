//go:build cgo && !tinygo && !hyperuuid_local && (darwin || linux || windows) && !ios && !android

package hyperuuid

// The core freshness check. Outside CI this suite links the committed archives under
// staticlib/, which stage-native-binaries.yml refreshes only at release, so after a change to
// the core a plain `go test ./...` would test the core as it was at the last staging. The
// check finds the commit that staging was built from and refuses to run when rust/ differs
// from it. CI writes a freshly built core over staticlib/ before the suite runs, which the
// check sees as a changed tree, and sets CI besides; the hyperuuid_local tag, which links the
// checkout's own core, leaves this file out. go test replays a cached pass without running
// TestMain, and it tracks no file outside this module, so a pass recorded before a change to
// rust/ is replayed after it: -count=1 runs the check again.

import (
	"fmt"
	"os"
	"os/exec"
	"regexp"
	"strings"
	"testing"
)

func TestMain(m *testing.M) {
	if message := staleCoreMessage(); message != "" {
		fmt.Fprintln(os.Stderr, message)
		os.Exit(1)
	}
	os.Exit(m.Run())
}

var stagedFrom = regexp.MustCompile(`\(([0-9a-f]{40})\)$`)

// staleCoreMessage returns why the committed core is older than this checkout's rust/, or ""
// when it is not, or when the question cannot be answered here.
func staleCoreMessage() string {
	const tree = "go/staticlib"
	if os.Getenv("CI") != "" {
		return ""
	}
	// git runs in the repository's top level once that is known, and reports its exit
	// code, or -1 when it could not run at all.
	top := ""
	git := func(args ...string) (string, int) {
		if top != "" {
			args = append([]string{"-C", top}, args...)
		}
		out, err := exec.Command("git", args...).Output()
		if exit, ok := err.(*exec.ExitError); ok {
			return "", exit.ExitCode()
		} else if err != nil {
			return "", -1
		}
		return strings.TrimSpace(string(out)), 0
	}
	dir, code := git("rev-parse", "--show-toplevel")
	if code != 0 || dir == "" {
		return ""
	}
	top = dir
	staging, _ := git("log", "-1", "--format=%H", "--grep=^chore: stage native binaries", "HEAD")
	if staging == "" {
		return ""
	}
	source, _ := git("log", "-1", "--format=%(trailers:key=Release-Source-SHA,valueonly)", staging)
	if source == "" {
		subject, _ := git("log", "-1", "--format=%s", staging)
		if match := stagedFrom.FindStringSubmatch(subject); match != nil {
			source = match[1]
		}
	}
	if len(source) < 12 {
		return ""
	}
	if _, code := git("cat-file", "-e", source+"^{commit}"); code != 0 {
		return ""
	}
	// Fresh binaries written into the tree count; its READMEs do not.
	if _, code := git("diff", "--quiet", "HEAD", "--", tree, ":(exclude,glob)"+tree+"/**/README.md"); code != 0 {
		return ""
	}
	_, code = git("diff", "--quiet", source, "--", "rust/src", "rust/Cargo.toml", "rust/Cargo.lock")
	untracked, _ := git("ls-files", "--others", "--exclude-standard", "--", "rust/src")
	if code != 1 && untracked == "" {
		return ""
	}
	return fmt.Sprintf("%s was built from rust/ at %s, and this checkout's rust/ differs from it, "+
		"so these tests would run against a stale core. Build the checkout's own with "+
		".github/scripts/local-core.sh (FORGE=<path> if the SkunkWerkx/.github checkout is not "+
		"../.github or ../HyperForge), then run %s instead; CI builds its own core and skips this check.",
		tree, source[:12], "go test -tags hyperuuid_local ./...")
}
