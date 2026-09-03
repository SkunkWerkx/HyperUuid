//go:build hyperuuid_wasm

// This backend runs the Rust core as a WebAssembly module inside the Go process, through
// github.com/bytecodealliance/wasmtime-go, instead of dlopen'ing a native build. The module
// is the same C-ABI surface ffi.rs exports, compiled for wasm32-wasip1 and embedded from
// native/wasm32-wasip1/hyperuuid.wasm alongside the per-platform shared libraries. It is the
// inverse of what the root README's WebAssembly table calls Go's "Structural" blocker: that
// row is about compiling *this Go module* to wasm, which neither cgo nor purego can do; this
// file is wasm running *inside* Go, which needs neither.
//
// Selected by build tag, never automatically: `go build -tags hyperuuid_wasm`. Reach for it
// on a platform this module ships no native build for, or when a deployment must not dlopen
// anything from a temp file at all (see native_extract.go). Two things to know before you
// do:
//
//   - wasmtime-go is cgo throughout — it links wasmtime's own precompiled static library
//     through the C API. That cuts against the purego story backend_purego.go exists for:
//     this backend needs a working C toolchain on every platform, Windows included, where
//     the default backend deliberately doesn't. The wasmtime-go module is also required in
//     go.mod unconditionally (Go has no tag-conditional requirements), so it lands in every
//     consumer's module graph; it only compiles into a binary built with the tag.
//   - Measured on go1.27 linux/arm64 in the session that added this: a single
//     uuid_new_v7 costs ~3.0 µs through wasmtime-go against 139 ns native, and a 1000-item
//     batch including the copy back out of guest memory ~42.6 µs against 18.4 µs. Per call
//     it is ~20x the native crossing; per UUID in a batch it is ~2.3x. Prefer the batch
//     and Fill doors here even more than usual.
//
// Memory protocol: a wasm guest sees only its own linear memory, so nothing here passes a Go
// pointer across. Buffers come from the module's exported malloc (wasi-libc's, which Rust's
// std allocator on this target already sits on) — never a host-picked offset past the data
// segments, because dlmalloc claims the tail of the initial memory on first use and a batch
// written there was observed corrupted by the guest's next allocation. Results are copied
// out of the guest into the caller's memory. One Store and one Instance serve the whole
// process, created lazily and serialized under a mutex (a wasmtime Store is not safe for
// concurrent use); the core's process-global v7 counter therefore lives in that one
// instance, exactly as it lives in one loaded shared library on the native backends.
package hyperuuid

import (
	"fmt"
	"sync"
	"unsafe"

	"github.com/bytecodealliance/wasmtime-go/v48"
	"github.com/google/uuid"
)

const wasmModulePath = "native/wasm32-wasip1/hyperuuid.wasm"

// wasmCore is the one embedded module instance the process shares. Every field after
// store is a guest export or a guest address; none is meaningful outside mu.
type wasmCore struct {
	mu    sync.Mutex
	store *wasmtime.Store
	mem   *wasmtime.Memory

	malloc, free *wasmtime.Func

	newV4, newV5, newV6, newV7, v6UnixMillis, v7UnixMillis,
	newV6Batch, newV7Batch, v7ToSql, v7ToRfc, v6ToSql, v6ToRfc *wasmtime.Func

	// Sixteen bytes in and sixteen out, malloc'd once for the single-UUID doors — the same
	// per-thread scratch the Java binding keeps, collapsed to one pair because every call
	// here already holds mu.
	in, out int32

	// Grow-only guest buffer for the v5 name and batch destinations: freed and re-malloc'd
	// only when a call needs more than it has, so a steady stream of same-sized batches
	// costs no allocator traffic at all.
	buf    int32
	bufLen int
}

var (
	initOnce sync.Once
	initErr  error
	core     *wasmCore
)

// ensureLoaded instantiates the embedded wasm module exactly once. The name and signature
// match the native backends so uuidgen.go needs no knowledge of which one it got.
func ensureLoaded() error {
	initOnce.Do(func() {
		core, initErr = newWasmCore()
	})
	return initErr
}

func newWasmCore() (*wasmCore, error) {
	wasm, err := nativeFS.ReadFile(wasmModulePath)
	if err != nil {
		return nil, fmt.Errorf("hyperuuid: %s not found in embedded native libs: %w", wasmModulePath, err)
	}

	engine := wasmtime.NewEngine()
	module, err := wasmtime.NewModule(engine, wasm)
	if err != nil {
		return nil, fmt.Errorf("hyperuuid: compiling wasm module: %w", err)
	}
	// The module imports five WASI preview1 functions (random_get for entropy, and the
	// environ/fd_write/proc_exit set wasi-libc's startup and panic paths reference); an
	// empty WasiConfig satisfies them — no files, no env, nothing inherited.
	linker := wasmtime.NewLinker(engine)
	if err := linker.DefineWasi(); err != nil {
		return nil, fmt.Errorf("hyperuuid: defining WASI imports: %w", err)
	}
	store := wasmtime.NewStore(engine)
	store.SetWasi(wasmtime.NewWasiConfig())
	instance, err := linker.Instantiate(store, module)
	if err != nil {
		return nil, fmt.Errorf("hyperuuid: instantiating wasm module: %w", err)
	}

	c := &wasmCore{store: store}
	memExport := instance.GetExport(store, "memory")
	if memExport == nil || memExport.Memory() == nil {
		return nil, fmt.Errorf("hyperuuid: wasm module exports no memory")
	}
	c.mem = memExport.Memory()

	fn := func(name string) (*wasmtime.Func, error) {
		f := instance.GetFunc(store, name)
		if f == nil {
			return nil, fmt.Errorf("hyperuuid: export %s not found in wasm module", name)
		}
		return f, nil
	}
	exports := []struct {
		name string
		dst  **wasmtime.Func
	}{
		{"malloc", &c.malloc}, {"free", &c.free},
		{"uuid_new_v4", &c.newV4}, {"uuid_new_v5", &c.newV5},
		{"uuid_new_v6", &c.newV6}, {"uuid_new_v7", &c.newV7},
		{"uuid_v6_unix_millis", &c.v6UnixMillis}, {"uuid_v7_unix_millis", &c.v7UnixMillis},
		{"uuid_new_v6_batch", &c.newV6Batch}, {"uuid_new_v7_batch", &c.newV7Batch},
		{"uuid_v7_to_sql_order", &c.v7ToSql}, {"uuid_v7_to_rfc_order", &c.v7ToRfc},
		{"uuid_v6_to_sql_order", &c.v6ToSql}, {"uuid_v6_to_rfc_order", &c.v6ToRfc},
	}
	for _, e := range exports {
		f, err := fn(e.name)
		if err != nil {
			return nil, err
		}
		*e.dst = f
	}

	if c.in, err = c.alloc(16); err != nil {
		return nil, err
	}
	if c.out, err = c.alloc(16); err != nil {
		return nil, err
	}
	return c, nil
}

// alloc asks the guest allocator for n bytes and returns the guest address.
func (c *wasmCore) alloc(n int) (int32, error) {
	v, err := c.malloc.Call(c.store, int32(n))
	if err != nil {
		return 0, fmt.Errorf("hyperuuid: guest malloc(%d) trapped: %w", n, err)
	}
	p, _ := v.(int32)
	if p == 0 {
		return 0, fmt.Errorf("hyperuuid: guest malloc(%d) returned null", n)
	}
	return p, nil
}

// scratch returns a guest buffer of at least n bytes, growing the cached one if needed.
func (c *wasmCore) scratch(n int) (int32, error) {
	if n <= c.bufLen {
		return c.buf, nil
	}
	if c.buf != 0 {
		if _, err := c.free.Call(c.store, c.buf); err != nil {
			return 0, fmt.Errorf("hyperuuid: guest free trapped: %w", err)
		}
		c.buf, c.bufLen = 0, 0
	}
	p, err := c.alloc(n)
	if err != nil {
		return 0, err
	}
	c.buf, c.bufLen = p, n
	return p, nil
}

// data is the guest's linear memory as a byte slice. Only valid until the next guest call
// that could grow memory (malloc can), so it is re-taken after every call, never cached.
func (c *wasmCore) data() []byte { return c.mem.UnsafeData(c.store) }

// call invokes a guest export and returns its single i32 result. A trap here is a bug in
// the core or this file (an out-of-bounds address), not a recoverable condition, so it
// panics the same way the native backends' AssertionError-class failures would.
func (c *wasmCore) call(f *wasmtime.Func, name string, args ...interface{}) int32 {
	v, err := f.Call(c.store, args...)
	if err != nil {
		panic(fmt.Sprintf("hyperuuid: %s trapped inside the wasm core: %v", name, err))
	}
	rc, _ := v.(int32)
	return rc
}

func (c *wasmCore) callU64(f *wasmtime.Func, name string, args ...interface{}) uint64 {
	v, err := f.Call(c.store, args...)
	if err != nil {
		panic(fmt.Sprintf("hyperuuid: %s trapped inside the wasm core: %v", name, err))
	}
	r, _ := v.(int64)
	return uint64(r)
}

func (c *wasmCore) readUUID(addr int32) uuid.UUID {
	var id uuid.UUID
	copy(id[:], c.data()[addr:addr+16])
	return id
}

func (c *wasmCore) writeUUID(addr int32, id uuid.UUID) {
	copy(c.data()[addr:addr+16], id[:])
}

// unixMillis crosses as the i64 the export's u64 parameter is spelled as in wasm.
func millisArg(unixMillis uint64) int64 { return int64(unixMillis) }

func newV4() (uuid.UUID, int32) {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	rc := c.call(c.newV4, "uuid_new_v4", c.out)
	return c.readUUID(c.out), rc
}

func newV5(ns uuid.UUID, name []byte) (uuid.UUID, int32) {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	// The core never dereferences name when name_len is 0, so an empty name reuses the
	// (never-read) input scratch rather than growing the buffer for nothing.
	namePtr := c.in
	if len(name) > 0 {
		p, err := c.scratch(len(name))
		if err != nil {
			panic(err)
		}
		copy(c.data()[p:int(p)+len(name)], name)
		namePtr = p
	}
	c.writeUUID(c.in, ns)
	rc := c.call(c.newV5, "uuid_new_v5", c.in, namePtr, int32(len(name)), c.out)
	return c.readUUID(c.out), rc
}

func newV6(unixMillis uint64) (uuid.UUID, int32) {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	rc := c.call(c.newV6, "uuid_new_v6", millisArg(unixMillis), c.out)
	return c.readUUID(c.out), rc
}

func newV7(unixMillis uint64) (uuid.UUID, int32) {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	rc := c.call(c.newV7, "uuid_new_v7", millisArg(unixMillis), c.out)
	return c.readUUID(c.out), rc
}

func unixMillisOf(f *wasmtime.Func, name string, id uuid.UUID) uint64 {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	c.writeUUID(c.in, id)
	return c.callU64(f, name, c.in)
}

func v6UnixMillis(id uuid.UUID) uint64 {
	return unixMillisOf(core.v6UnixMillis, "uuid_v6_unix_millis", id)
}
func v7UnixMillis(id uuid.UUID) uint64 {
	return unixMillisOf(core.v7UnixMillis, "uuid_v7_unix_millis", id)
}

// batch fills count*16 bytes at out (Go memory) from one guest batch call: the guest writes
// into its own scratch, and the result is copied out under the lock.
func batch(f *wasmtime.Func, name string, unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	n := int(count) * 16
	p, err := c.scratch(n)
	if err != nil {
		panic(err)
	}
	rc := c.call(f, name, millisArg(unixMillis), int32(count), p)
	if rc == 0 {
		copy(unsafe.Slice((*byte)(out), n), c.data()[p:int(p)+n])
	}
	return rc
}

func newV6Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return batch(core.newV6Batch, "uuid_new_v6_batch", unixMillis, count, out)
}

func newV7Batch(unixMillis uint64, count uint32, out unsafe.Pointer) int32 {
	return batch(core.newV7Batch, "uuid_new_v7_batch", unixMillis, count, out)
}

// reorder runs one of the in-place byte-order transforms over sixteen bytes staged in the
// guest's input scratch and returns the rewritten sixteen.
func reorder(f *wasmtime.Func, name string, id uuid.UUID) uuid.UUID {
	c := core
	c.mu.Lock()
	defer c.mu.Unlock()
	c.writeUUID(c.in, id)
	if _, err := f.Call(c.store, c.in); err != nil {
		panic(fmt.Sprintf("hyperuuid: %s trapped inside the wasm core: %v", name, err))
	}
	return c.readUUID(c.in)
}

func v7ToSqlOrder(id uuid.UUID) uuid.UUID { return reorder(core.v7ToSql, "uuid_v7_to_sql_order", id) }
func v7ToRfcOrder(id uuid.UUID) uuid.UUID { return reorder(core.v7ToRfc, "uuid_v7_to_rfc_order", id) }
func v6ToSqlOrder(id uuid.UUID) uuid.UUID { return reorder(core.v6ToSql, "uuid_v6_to_sql_order", id) }
func v6ToRfcOrder(id uuid.UUID) uuid.UUID { return reorder(core.v6ToRfc, "uuid_v6_to_rfc_order", id) }

// reorderBytes is the same transform over a caller's own sixteen bytes in Go memory,
// which cross by copy in both directions here rather than by pointer.
func reorderBytes(f *wasmtime.Func, name string, p unsafe.Pointer) {
	b := unsafe.Slice((*byte)(p), 16)
	id := reorder(f, name, uuid.UUID(b))
	copy(b, id[:])
}

func v7ToSqlOrderBytes(p unsafe.Pointer) { reorderBytes(core.v7ToSql, "uuid_v7_to_sql_order", p) }
func v7ToRfcOrderBytes(p unsafe.Pointer) { reorderBytes(core.v7ToRfc, "uuid_v7_to_rfc_order", p) }
func v6ToSqlOrderBytes(p unsafe.Pointer) { reorderBytes(core.v6ToSql, "uuid_v6_to_sql_order", p) }
func v6ToRfcOrderBytes(p unsafe.Pointer) { reorderBytes(core.v6ToRfc, "uuid_v6_to_rfc_order", p) }
