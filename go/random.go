package hyperuuid

import (
	"crypto/rand"
	"io"
	"time"
)

// A source serves the core's WASI random_get: it fills dst and returns a WASI errno, 0 on
// success. The core turns a nonzero errno into its random-source failure code, which the
// caller sees as ErrRandomSource; a source never returns 0 over bytes it did not write.
type source interface {
	fill(dst []byte) int32
}

// wasiEIO is WASI's EIO, the errno a source returns when the operating system's reader
// fails or falls short.
const wasiEIO int32 = 29

// entropy is the one reader both sources draw from: the operating system through
// crypto/rand. Stock Go's Read never returns an error (since Go 1.24 it crashes instead),
// TinyGo's can fail or fall short, so every draw goes through readFull and is checked.
// Tests swap it (withEntropy); nothing else writes it.
var entropy io.Reader = rand.Reader

// readFull fills dst from entropy, retrying short successful reads as io.ReadFull does, and
// reports whether every byte arrived.
func readFull(dst []byte) bool {
	_, err := io.ReadFull(entropy, dst)
	return err == nil
}

// osSource is one checked read of the operating system per random_get. It is the source
// wherever the AES stream is not selected, and the source of every one-off v5 instance.
type osSource struct{}

func (osSource) fill(dst []byte) int32 {
	if len(dst) == 0 {
		return 0
	}
	if !readFull(dst) {
		return wasiEIO
	}
	return 0
}

// acceleratedOverride, when set, replaces accelerated()'s answer. Tests set it
// (withAccelerated); in production it is nil.
var acceleratedOverride func() bool

// newSource picks an instance's source at creation: the AES-256-CTR stream when Go's AES is
// accelerated on this CPU and FIPS 140 mode is off, else the operating system.
func newSource(now func() time.Time) source {
	if s := newStream(now); s != nil {
		return s
	}
	return osSource{}
}
