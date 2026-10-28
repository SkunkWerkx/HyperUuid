//go:build cgo && !tinygo && !hyperuuid_local && (darwin || linux || windows) && !ios && !android

package hyperuuid

// The core freshness check lives in internal/corefreshness, shared with the exhaustion suite
// (internal/exhaustiontest); see there for what it checks and when it stays out of the way.
// go test replays a cached pass without running TestMain, and it tracks no file outside this
// module, so a pass recorded before a change to rust/ is replayed after it: -count=1 runs the
// check again. The hyperuuid_local tag, which links the checkout's own core, leaves this file
// out.

import (
	"fmt"
	"os"
	"testing"

	"github.com/SkunkWerkx/HyperUuid/go/internal/corefreshness"
)

func TestMain(m *testing.M) {
	if message := corefreshness.StaleCoreMessage(); message != "" {
		fmt.Fprintln(os.Stderr, message)
		os.Exit(1)
	}
	os.Exit(m.Run())
}
