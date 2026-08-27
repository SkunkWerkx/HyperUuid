package hyperuuid

import (
	"fmt"
	"runtime"
)

// target names the RID-style directory (matching the C#/Java bindings' runtimes/{rid}/
// convention) and native library filename for the running GOOS/GOARCH.
type target struct {
	rid     string
	libName string
}

// currentTarget maps the running GOOS/GOARCH to the embedded native library it should load,
// mirroring the Java binding's NativePlatform.detect().
func currentTarget() (target, error) {
	isArm := runtime.GOARCH == "arm64"

	switch runtime.GOOS {
	case "windows":
		if isArm {
			return target{"win-arm64", "hyperuuid.dll"}, nil
		}
		return target{"win-x64", "hyperuuid.dll"}, nil
	case "darwin":
		if isArm {
			return target{"osx-arm64", "libhyperuuid.dylib"}, nil
		}
		return target{"osx-x64", "libhyperuuid.dylib"}, nil
	case "linux":
		if isArm {
			return target{"linux-arm64", "libhyperuuid.so"}, nil
		}
		return target{"linux-x64", "libhyperuuid.so"}, nil
	default:
		return target{}, fmt.Errorf("hyperuuid: unsupported platform GOOS=%s GOARCH=%s", runtime.GOOS, runtime.GOARCH)
	}
}
