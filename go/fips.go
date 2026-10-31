//go:build !tinygo

package hyperuuid

import "crypto/fips140"

// fipsEnabled reports whether the program runs in FIPS 140-3 mode (GODEBUG=fips140=on or
// the fips140 build setting). The AES stream is never selected there: the operating
// system's reader is the approved source.
func fipsEnabled() bool {
	return fips140.Enabled()
}
