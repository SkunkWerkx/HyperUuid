//go:build cgo && !tinygo && !hyperuuid_local && (darwin || linux || windows) && !ios && !android

package exhaustiontest

// The same refusal, under the same tag, as the root package's: outside CI a plain go test
// links the committed core, which may predate the one that can exhaust the stream.

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
