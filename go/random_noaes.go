//go:build !((amd64 || arm64 || s390x) && !purego && !tinygo)

package hyperuuid

import "time"

// The inverse of random_aes.go's tag: every architecture where Go's AES is not assembly
// (ppc64 and ppc64le included: Go assumes POWER8 there with no runtime check, so
// acceleration cannot be established from outside), every purego build, and TinyGo. The
// operating system is the source.

func accelerated() bool {
	if acceleratedOverride != nil {
		return acceleratedOverride()
	}
	return false
}

func newStream(func() time.Time) source {
	return nil
}
