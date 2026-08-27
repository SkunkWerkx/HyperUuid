//go:build windows

package hyperuuid

import "syscall"

// purego.Dlopen/RTLD_NOW/RTLD_GLOBAL don't exist on windows — purego's own official example
// (examples/libc/main_windows.go) opens the library via stdlib syscall.LoadLibrary instead,
// to avoid an extra dependency; purego.RegisterLibFunc's symbol lookup (syscall.GetProcAddress
// under the hood) already works the same for whatever uintptr handle this returns.
func openLibrary(path string) (uintptr, error) {
	handle, err := syscall.LoadLibrary(path)
	return uintptr(handle), err
}
