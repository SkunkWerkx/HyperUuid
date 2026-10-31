//go:build (amd64 || arm64 || s390x) && !purego && !tinygo

package hyperuuid

import (
	"crypto/aes"
	"crypto/cipher"
	"runtime"
	"time"

	"golang.org/x/sys/cpu"
)

// The AES-256-CTR stream: a keystream buffer that serves the core's random_get where Go's
// AES is hardware-accelerated, so a v7 batch of 1000 draws 6,004 bytes from the cipher
// instead of from the operating system. It is selected only when accelerated() and FIPS 140
// mode is off (newStream); everywhere else the operating system is the source (random.go).

const (
	// keyBudget is the keystream generated under one key before a rekey: refills and
	// direct generation counted alike, enforced inside a draw (a generation that would
	// cross it is cut at the boundary and finished under the new key).
	keyBudget = 1 << 20
	// keyLifetime is how long a key serves. Checked at the entry of every draw, before any
	// buffered byte is served, so buffered keystream never outlives its key's second.
	keyLifetime = time.Second
	// bufSize is the keystream buffer. A draw of at least this much left over after the
	// buffer is drained is generated directly into the destination.
	bufSize = 512
)

// accelerated reports whether Go's AES is the assembly implementation on this CPU: the
// same features Go's own selection requires, read through golang.org/x/sys/cpu, which also
// honours GODEBUG=cpu.aes=off. The rule is Go 1.27's, crypto/internal/fips140/aes/aes_asm.go
// line 24 (`supportsAES = cpu.X86HasAES && cpu.X86HasSSE41 && cpu.X86HasSSSE3 ||
// cpu.ARM64HasAES || cpu.PPC64 || cpu.PPC64le`) and aes_s390x.go line 38
// (`cpu.S390XHasAES && cpu.S390XHasAESCBC`); re-read it at every Go floor bump. ppc64 and
// ppc64le are not here: Go assumes POWER8 there with no runtime detection and only an
// unregistered GODEBUG knob to turn it off, so acceleration cannot be established from
// outside and they take the operating system (random_noaes.go). The generic fallback that
// a false answer avoids is variable-time software AES.
func accelerated() bool {
	if acceleratedOverride != nil {
		return acceleratedOverride()
	}
	switch runtime.GOARCH {
	case "amd64":
		return cpu.X86.HasAES && cpu.X86.HasSSE41 && cpu.X86.HasSSSE3
	case "arm64":
		return cpu.ARM64.HasAES
	case "s390x":
		return cpu.S390X.HasAES && cpu.S390X.HasAESCBC
	}
	return false
}

// newStream returns the AES stream when it is the right source here, else nil.
func newStream(now func() time.Time) source {
	if !accelerated() || fipsEnabled() {
		return nil
	}
	return newAESSource(now)
}

type aesSource struct {
	now       func() time.Time
	key       [32]byte // reusable storage: a rekey allocates the cipher and the CTR, nothing else
	iv        [16]byte // always zero: every key is fresh, so the counter starts at zero under it
	stream    cipher.Stream
	keyedAt   time.Time
	generated int // keystream bytes generated under the current key
	buf       [bufSize]byte
	off       int // buf[off:] is unserved keystream; off == bufSize means empty
}

func newAESSource(now func() time.Time) *aesSource {
	return &aesSource{now: now, off: bufSize}
}

// fill serves len(dst) bytes of one contiguous keystream: the buffer's remainder first, then
// a direct generation when at least bufSize is still wanted, else a refill. It returns
// wasiEIO when a rekey's key cannot be read, with no claim over any byte already written.
func (s *aesSource) fill(dst []byte) int32 {
	if len(dst) == 0 {
		return 0
	}
	if s.stream == nil || s.now().Sub(s.keyedAt) >= keyLifetime {
		if !s.rekey() {
			return wasiEIO
		}
	}
	n := copy(dst, s.buf[s.off:])
	s.off += n
	dst = dst[n:]
	if len(dst) >= bufSize {
		clear(dst) // the stored bytes are keystream, not keystream XOR whatever was there
		return s.generate(dst)
	}
	if len(dst) > 0 {
		clear(s.buf[:])
		if code := s.generate(s.buf[:]); code != 0 {
			s.off = bufSize
			return code
		}
		s.off = copy(dst, s.buf[:])
	}
	return 0
}

// generate XORs keystream over dst (zeroed by the caller) under the byte budget: a
// generation that would cross keyBudget is cut there, the stream is rekeyed, and the rest
// is generated under the new key.
func (s *aesSource) generate(dst []byte) int32 {
	for len(dst) > 0 {
		if s.generated >= keyBudget {
			if !s.rekey() {
				return wasiEIO
			}
		}
		n := min(len(dst), keyBudget-s.generated)
		s.stream.XORKeyStream(dst[:n], dst[:n])
		s.generated += n
		dst = dst[n:]
	}
	return 0
}

// rekey discards the buffer and the current key, reads a fresh key through the checked
// reader, and starts the counter at zero. On a failed read the source is left keyless:
// nothing is served under the old key or from the old buffer until a later rekey completes.
func (s *aesSource) rekey() bool {
	s.stream = nil
	s.off = bufSize
	s.generated = 0
	if !readFull(s.key[:]) {
		return false
	}
	block, err := aes.NewCipher(s.key[:])
	if err != nil {
		return false // unreachable: a 32-byte key is always valid
	}
	s.stream = cipher.NewCTR(block, s.iv[:])
	s.keyedAt = s.now()
	return true
}
