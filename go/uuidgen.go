// Package hyperuuid provides RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7
// time-sortable) calling directly into the native libhyperuuid shared library via
// github.com/ebitengine/purego — dlopen/dlsym plus per-arch call trampolines, no cgo and no C
// compiler required to build or consume this module (the same "no runtime bridge" positioning
// as the Python/ctypes and Java/FFM bindings).
//
// This module bundles a native build for every supported platform (see currentTarget) and
// loads the right one at runtime, the same trick the Java binding uses since neither a Go
// module nor a .jar has NuGet-style per-RID package selection.
package hyperuuid

import (
	"fmt"
	"os"
	"sync"
	"time"
	"unsafe"

	"github.com/ebitengine/purego"
	"github.com/google/uuid"
)

// Well-known namespace UUIDs defined in RFC 9562 Section 6.6. These are byte-identical to
// google/uuid's own namespace constants, so they're just re-exported for API-shape symmetry
// with the other bindings' Namespaces.* members.
var (
	NamespaceDNS  = uuid.NameSpaceDNS
	NamespaceURL  = uuid.NameSpaceURL
	NamespaceOID  = uuid.NameSpaceOID
	NamespaceX500 = uuid.NameSpaceX500
)

// The RFC 9562 §5.9 Nil and §5.10 Max UUIDs. Same story as the Namespace* vars above —
// byte-identical to google/uuid's own Nil/Max, re-exported for API-shape symmetry.
var (
	Nil = uuid.Nil
	Max = uuid.Max
)

var (
	initOnce sync.Once
	initErr  error

	uuidNewV4        func(out unsafe.Pointer) int32
	uuidNewV5        func(ns, name unsafe.Pointer, nameLen uint32, out unsafe.Pointer) int32
	uuidNewV6        func(unixMillis uint64, out unsafe.Pointer) int32
	uuidV6UnixMillis func(uuid unsafe.Pointer) uint64
	uuidNewV6Batch   func(unixMillis uint64, count uint32, out unsafe.Pointer) int32
	uuidNewV7        func(unixMillis uint64, out unsafe.Pointer) int32
	uuidV7UnixMillis func(uuid unsafe.Pointer) uint64
	uuidNewV7Batch   func(unixMillis uint64, count uint32, out unsafe.Pointer) int32
	uuidV7ToSqlOrder func(uuid unsafe.Pointer)
	uuidV7ToRfcOrder func(uuid unsafe.Pointer)
)

// ensureLoaded extracts this platform's embedded native library to a temp file and dlopen's
// it, exactly once. The temp file is deliberately never removed afterward — Go has no
// reliable process-exit hook, the same best-effort tradeoff the Java binding makes with
// File.deleteOnExit (itself not guaranteed, e.g. on kill -9).
func ensureLoaded() error {
	initOnce.Do(func() {
		t, err := currentTarget()
		if err != nil {
			initErr = err
			return
		}

		resourcePath := "native/" + t.rid + "/" + t.libName
		data, err := nativeFS.ReadFile(resourcePath)
		if err != nil {
			initErr = fmt.Errorf("hyperuuid: %s not found in embedded native libs (unsupported platform, or this module was built without a native library for it): %w", resourcePath, err)
			return
		}

		tmp, err := os.CreateTemp("", "libhyperuuid-*-"+t.libName)
		if err != nil {
			initErr = fmt.Errorf("hyperuuid: creating temp file for native library: %w", err)
			return
		}
		_, writeErr := tmp.Write(data)
		closeErr := tmp.Close()
		// The write handle must be closed before openLibrary (dlopen/LoadLibrary) opens the
		// same path — Windows enforces exclusive file access far more strictly than Unix, and
		// LoadLibrary fails outright while a write handle on the same file is still open.
		if writeErr != nil {
			initErr = fmt.Errorf("hyperuuid: writing native library to temp file: %w", writeErr)
			return
		}
		if closeErr != nil {
			initErr = fmt.Errorf("hyperuuid: closing temp file for native library: %w", closeErr)
			return
		}

		handle, err := openLibrary(tmp.Name())
		if err != nil {
			initErr = fmt.Errorf("hyperuuid: loading native library: %w", err)
			return
		}

		purego.RegisterLibFunc(&uuidNewV4, handle, "uuid_new_v4")
		purego.RegisterLibFunc(&uuidNewV5, handle, "uuid_new_v5")
		purego.RegisterLibFunc(&uuidNewV6, handle, "uuid_new_v6")
		purego.RegisterLibFunc(&uuidV6UnixMillis, handle, "uuid_v6_unix_millis")
		purego.RegisterLibFunc(&uuidNewV6Batch, handle, "uuid_new_v6_batch")
		purego.RegisterLibFunc(&uuidNewV7, handle, "uuid_new_v7")
		purego.RegisterLibFunc(&uuidV7UnixMillis, handle, "uuid_v7_unix_millis")
		purego.RegisterLibFunc(&uuidNewV7Batch, handle, "uuid_new_v7_batch")
		purego.RegisterLibFunc(&uuidV7ToSqlOrder, handle, "uuid_v7_to_sql_order")
		purego.RegisterLibFunc(&uuidV7ToRfcOrder, handle, "uuid_v7_to_rfc_order")
	})
	return initErr
}

// NewV4 creates a random UUID version 4 (RFC 9562 §5.4).
func NewV4() (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	var out uuid.UUID
	if rc := uuidNewV4(unsafe.Pointer(&out[0])); rc != 0 {
		return uuid.UUID{}, fmt.Errorf("uuid_new_v4 failed with code %d: %w", rc, ErrRandomSource)
	}
	return out, nil
}

// NewV5 creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
// bytes. The same (namespace, name) pair always produces the same UUID.
func NewV5(namespace uuid.UUID, name []byte) (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	var out uuid.UUID
	var namePtr unsafe.Pointer
	if len(name) > 0 {
		namePtr = unsafe.Pointer(&name[0])
	}
	if rc := uuidNewV5(unsafe.Pointer(&namespace[0]), namePtr, uint32(len(name)), unsafe.Pointer(&out[0])); rc != 0 {
		return uuid.UUID{}, fmt.Errorf("uuid_new_v5 failed with code %d: %w", rc, ErrRandomSource)
	}
	return out, nil
}

// NewV5String creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a
// UTF-8 name.
func NewV5String(namespace uuid.UUID, name string) (uuid.UUID, error) {
	return NewV5(namespace, []byte(name))
}

// NewV6 creates a time-sortable UUID version 6 (RFC 9562 §5.6), a field-compatible
// reordering of version 1 for better sort/index locality, using the current time.
func NewV6() (uuid.UUID, error) {
	return NewV6At(uint64(time.Now().UnixMilli()))
}

// NewV6At creates a time-sortable UUID version 6 (RFC 9562 §5.6), embedding unixMillis
// (milliseconds since the Unix epoch). clock_seq and node are randomly generated on every
// call — unlike version 7, there is no monotonic counter, so calls within the same
// millisecond are not guaranteed to sort in creation order.
func NewV6At(unixMillis uint64) (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	var out uuid.UUID
	switch rc := uuidNewV6(unixMillis, unsafe.Pointer(&out[0])); rc {
	case 0:
		return out, nil
	case 2:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v6 failed with code %d: %w", rc, ErrTimestampOutOfRange)
	default:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v6 failed with code %d: %w", rc, ErrRandomSource)
	}
}

// NewV6Batch creates count time-sortable version 6 UUIDs sharing one timestamp capture,
// using the current time — one native call and one random-bytes fetch instead of count of
// each.
func NewV6Batch(count int) ([]uuid.UUID, error) {
	return NewV6BatchAt(count, uint64(time.Now().UnixMilli()))
}

// NewV6BatchAt creates count time-sortable version 6 UUIDs sharing one unixMillis timestamp
// capture. clock_seq and node are independently random per item — unlike version 7, there is
// no monotonic counter, so items are not guaranteed to sort in creation order.
func NewV6BatchAt(count int, unixMillis uint64) ([]uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return nil, err
	}
	if count == 0 {
		return nil, nil
	}
	buf := make([]byte, count*16)
	switch rc := uuidNewV6Batch(unixMillis, uint32(count), unsafe.Pointer(&buf[0])); rc {
	case 0:
		// fall through to conversion below
	case 2:
		return nil, fmt.Errorf("uuid_new_v6_batch failed with code %d: %w", rc, ErrTimestampOutOfRange)
	default:
		return nil, fmt.Errorf("uuid_new_v6_batch failed with code %d: %w", rc, ErrRandomSource)
	}
	result := make([]uuid.UUID, count)
	for i := range result {
		copy(result[i][:], buf[i*16:(i+1)*16])
	}
	return result, nil
}

// V6UnixMillis recovers the Unix-epoch millisecond timestamp embedded in a version 6 UUID's
// timestamp field. Only meaningful when id.Version() == 6 — the RFC 9562 bit layout doesn't
// distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp", so the caller is
// responsible for checking that first if it matters.
func V6UnixMillis(id uuid.UUID) (uint64, error) {
	if err := ensureLoaded(); err != nil {
		return 0, err
	}
	return uuidV6UnixMillis(unsafe.Pointer(&id[0])), nil
}

// V6Timestamp recovers the UTC timestamp embedded in a version 6 UUID as a time.Time.
func V6Timestamp(id uuid.UUID) (time.Time, error) {
	millis, err := V6UnixMillis(id)
	if err != nil {
		return time.Time{}, err
	}
	return time.UnixMilli(int64(millis)).UTC(), nil
}

// V7UnixMillis recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
// unix_ts_ms field. Only meaningful when id.Version() == 7 — the RFC 9562 bit layout doesn't
// distinguish "not a v7 UUID" from "v7 UUID with a very early timestamp", so the caller is
// responsible for checking that first if it matters.
func V7UnixMillis(id uuid.UUID) (uint64, error) {
	if err := ensureLoaded(); err != nil {
		return 0, err
	}
	return uuidV7UnixMillis(unsafe.Pointer(&id[0])), nil
}

// V7Timestamp recovers the UTC timestamp embedded in a version 7 UUID as a time.Time.
func V7Timestamp(id uuid.UUID) (time.Time, error) {
	millis, err := V7UnixMillis(id)
	if err != nil {
		return time.Time{}, err
	}
	return time.UnixMilli(int64(millis)).UTC(), nil
}

// NewV7 creates a time-sortable UUID version 7 (RFC 9562 §6.2) using the current time.
func NewV7() (uuid.UUID, error) {
	return NewV7At(uint64(time.Now().UnixMilli()))
}

// NewV7At creates a time-sortable UUID version 7 (RFC 9562 §6.2), embedding unixMillis
// (milliseconds since the Unix epoch).
func NewV7At(unixMillis uint64) (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	var out uuid.UUID
	switch rc := uuidNewV7(unixMillis, unsafe.Pointer(&out[0])); rc {
	case 0:
		return out, nil
	case 2:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v7 failed with code %d: %w", rc, ErrTimestampOutOfRange)
	default:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v7 failed with code %d: %w", rc, ErrRandomSource)
	}
}

// NewV7Batch creates count time-sortable version 7 UUIDs sharing one timestamp capture and
// one contiguous block of the monotonic counter, using the current time — one native call and
// one random-bytes fetch instead of count of each.
func NewV7Batch(count int) ([]uuid.UUID, error) {
	return NewV7BatchAt(count, uint64(time.Now().UnixMilli()))
}

// NewV7BatchAt creates count time-sortable version 7 UUIDs sharing one unixMillis timestamp
// capture and one contiguous block of the monotonic counter.
func NewV7BatchAt(count int, unixMillis uint64) ([]uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return nil, err
	}
	if count == 0 {
		return nil, nil
	}
	buf := make([]byte, count*16)
	switch rc := uuidNewV7Batch(unixMillis, uint32(count), unsafe.Pointer(&buf[0])); rc {
	case 0:
		// fall through to conversion below
	case 2:
		return nil, fmt.Errorf("uuid_new_v7_batch failed with code %d: %w", rc, ErrTimestampOutOfRange)
	default:
		return nil, fmt.Errorf("uuid_new_v7_batch failed with code %d: %w", rc, ErrRandomSource)
	}
	result := make([]uuid.UUID, count)
	for i := range result {
		copy(result[i][:], buf[i*16:(i+1)*16])
	}
	return result, nil
}

// ToSqlOrder converts an RFC 9562-ordered version 7 id to the byte order SQL Server's
// uniqueidentifier needs on the wire to sort by creation order.
//
// System.Data.SqlTypes.SqlGuid comparison — and therefore T-SQL ORDER BY on a
// uniqueidentifier column — doesn't compare a GUID's 16 bytes left to right; it uses a fixed,
// non-sequential byte significance order (most-significant first: octets 10,11,12,13,14,15,
// 8,9, 6,7, 4,5, 0,1,2,3). This moves the timestamp and counter — the two fields that
// determine creation order — into those most-significant octets, and moves the trailing
// entropy, which carries no ordering information, into the least-significant ones as one
// intact block. The permutation is computed once in the native Rust core and verified there
// (and independently, against the real System.Data.SqlTypes.SqlGuid comparator, in this
// project's C# test suite); this binding calls the same native function rather than
// reimplementing the math. Meaningful only for a genuine version 7 UUID.
func ToSqlOrder(id uuid.UUID) (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	out := id
	uuidV7ToSqlOrder(unsafe.Pointer(&out[0]))
	return out, nil
}

// FromSqlOrder is the inverse of ToSqlOrder — converts a SQL-Server-ordered version 7 id back
// to RFC 9562 order.
func FromSqlOrder(id uuid.UUID) (uuid.UUID, error) {
	if err := ensureLoaded(); err != nil {
		return uuid.UUID{}, err
	}
	out := id
	uuidV7ToRfcOrder(unsafe.Pointer(&out[0]))
	return out, nil
}
