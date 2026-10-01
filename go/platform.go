package hyperuuid

import (
	"bytes"
	"fmt"
	"os"
	"runtime"
)

// target names the RID-style directory (matching the C#/Java bindings' runtimes/{rid}/
// convention) and native library filename for the running GOOS/GOARCH.
type target struct {
	rid     string
	libName string
}

// currentTarget maps the running GOOS/GOARCH — and, on Linux, the libc this process is
// actually running on — to the embedded native library it should load, mirroring the Java
// binding's NativePlatform.detect().
func currentTarget() (target, error) {
	return targetFor(runtime.GOOS, runtime.GOARCH, runtime.GOOS == "linux" && runningOnMusl())
}

// targetFor is currentTarget with its inputs spelled out, so the mapping can be tested for
// platforms other than the one running the tests. An architecture this module ships no
// build for is an error here rather than a guess: defaulting to x64 would hand the loader
// a library it can only reject with a "wrong ELF class" message that names neither the
// platform nor the fix.
func targetFor(goos, goarch string, musl bool) (target, error) {
	var arch string
	switch goarch {
	case "amd64":
		arch = "x64"
	case "arm64":
		arch = "arm64"
	default:
		return target{}, fmt.Errorf("unsupported platform %s/%s", goos, goarch)
	}

	switch goos {
	case "windows":
		return target{"win-" + arch, "hyperuuid.dll"}, nil
	case "darwin":
		return target{"osx-" + arch, "libhyperuuid.dylib"}, nil
	case "linux":
		if musl {
			return target{"linux-musl-" + arch, "libhyperuuid.so"}, nil
		}
		return target{"linux-" + arch, "libhyperuuid.so"}, nil
	default:
		return target{}, fmt.Errorf("unsupported platform %s/%s", goos, goarch)
	}
}

// runningOnMusl reports whether this process is running on musl libc (Alpine and friends)
// rather than glibc, which decides between the linux-musl-* and linux-* builds: a shared
// library built against one libc does not load under the other. The rule — the process has
// a musl loader mapped — asks what the process is running on rather than what the
// distribution has installed. An unreadable /proc/self/maps means glibc.
func runningOnMusl() bool {
	maps, err := os.ReadFile("/proc/self/maps")
	if err != nil {
		return false
	}
	return mapsMentionMusl(maps)
}

// mapsMentionMusl is the test over /proc/self/maps' contents: musl's dynamic loader and its
// libc are one file, mapped as ld-musl-{arch}.so.1 or, through its usual symlink,
// libc.musl-{arch}.so.1.
func mapsMentionMusl(maps []byte) bool {
	return bytes.Contains(maps, []byte("ld-musl-")) || bytes.Contains(maps, []byte("libc.musl-"))
}
