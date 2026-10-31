// Package exhaustiontest drives the real v7 stream to its ceiling through the public API.
//
// The stream is process-global and driving it to the ceiling poisons it for every call after,
// so this is a package of its own: a separate package is a separate test binary, which is a
// process of its own, with no other test sharing the stream.
package exhaustiontest

import (
	"errors"
	"testing"

	hyperuuid "github.com/SkunkWerkx/HyperUuid/go"
	"github.com/google/uuid"
)

// m is 2^44 − 2, the last millisecond the stream holds.
const m uint64 = 17592186044414

const message = "hyperuuid: version 7 counter exhausted: the stream cannot advance without running " +
	"more than 1000 ms ahead of the supplied timestamp or past its last millisecond"

// exhausted reports whether err is the exhaustion error, failing on any other error and on
// the message not being the exact one.
func exhausted(t *testing.T, step string, err error) bool {
	t.Helper()
	if err == nil {
		return false
	}
	if !errors.Is(err, hyperuuid.ErrCounterExhausted) {
		t.Fatalf("%s: got %v, want ErrCounterExhausted", step, err)
	}
	if hyperuuid.ErrCounterExhausted.Error() != message {
		t.Fatalf("%s: message = %q, want %q", step, hyperuuid.ErrCounterExhausted.Error(), message)
	}
	return true
}

func mustExhaust(t *testing.T, step string, err error) {
	t.Helper()
	if !exhausted(t, step, err) {
		t.Fatalf("%s: succeeded, want ErrCounterExhausted", step)
	}
}

func TestCounterExhaustion(t *testing.T) {
	// 1. One single v7 at M succeeds.
	if _, err := hyperuuid.NewV7At(m); err != nil {
		t.Fatalf("single at M: %v", err)
	}

	// 2. Batches of 65,536 at M until the first error: the batch site.
	batchFailed := false
	for i := 0; i < 1025 && !batchFailed; i++ {
		_, err := hyperuuid.NewV7BatchAt(65536, m)
		batchFailed = exhausted(t, "batch site", err)
	}
	if !batchFailed {
		t.Fatal("no batch exhausted the stream")
	}

	// 3. Singles at M until the first error: the single site.
	singleFailed := false
	for i := 0; i < 1024 && !singleFailed; i++ {
		_, err := hyperuuid.NewV7At(m)
		singleFailed = exhausted(t, "single site", err)
	}
	if !singleFailed {
		t.Fatal("no single exhausted the stream")
	}

	// 4. The stream now sits at its ceiling, so every in-stream call at M fails at once,
	// through every other v7 door.
	_, err := hyperuuid.NewV7At(m)
	mustExhaust(t, "NewV7At", err)
	mustExhaust(t, "FillV7At", hyperuuid.FillV7At(make([]uuid.UUID, 1), m))
	mustExhaust(t, "FillV7BytesAt", hyperuuid.FillV7BytesAt(make([]byte, 16), m))
	_, err = hyperuuid.NewV7BatchAt(1, m)
	mustExhaust(t, "NewV7BatchAt", err)
}
