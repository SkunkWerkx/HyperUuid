//go:build tinygo

package hyperuuid

// TinyGo has no crypto/fips140, and the AES stream is never built for it (random_aes.go's
// tag), so the question does not arise; the operating system is the source.
func fipsEnabled() bool {
	return false
}
