// Package hyperuuid provides RFC 9562 UUID generation (v4 random, v5 deterministic, v7
// time-sortable, with a version 8 SQL Server order for v7) by calling the Rust core, linked into the binary as a static library through
// cgo (backend_static.go). It builds on Linux, macOS and Windows, on amd64 and arm64, with a
// C compiler present, and under TinyGo for WebAssembly, browser included (backend_tinygo.go);
// anything else is a compile error that says so (unsupported.go).
//
// The core is linked, not loaded, so nothing here can fail to load: Available is always
// true, and the errors these functions return are the core's own — a failed random source,
// a timestamp out of range — or a caller's mistake, such as a negative count.
package hyperuuid

import (
	"fmt"
	"math"
	"time"
	"unsafe"

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

// NewV4 creates a random UUID version 4 (RFC 9562 §5.4).
func NewV4() (uuid.UUID, error) {
	out, rc := newV4()
	if rc != 0 {
		return uuid.UUID{}, fmt.Errorf("uuid_new_v4 failed with code %d: %w", rc, ErrRandomSource)
	}
	return out, nil
}

// NewV5 creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and raw name
// bytes. The same (namespace, name) pair always produces the same UUID; an empty or nil name
// is valid and hashes the namespace alone.
func NewV5(namespace uuid.UUID, name []byte) (uuid.UUID, error) {
	if uint64(len(name)) > math.MaxUint32 {
		return uuid.UUID{}, fmt.Errorf("hyperuuid: name of %d bytes exceeds the native length limit", len(name))
	}
	// uuid_new_v5 is pure hashing — no entropy, no clock — and has no failure code: it
	// returns 0 unconditionally, so there is nothing to map onto an error here.
	out, _ := newV5(namespace, name)
	return out, nil
}

// NewV5String creates a deterministic UUID version 5 (RFC 9562 §5.5) from a namespace and a
// UTF-8 name.
func NewV5String(namespace uuid.UUID, name string) (uuid.UUID, error) {
	return NewV5(namespace, []byte(name))
}

// V7UnixMillis recovers the Unix-epoch millisecond timestamp embedded in a version 7 UUID's
// unix_ts_ms field. Only meaningful when id.Version() == 7 — the RFC 9562 bit layout doesn't
// distinguish "not a v7 UUID" from "v7 UUID with a very early timestamp", so the caller is
// responsible for checking that first if it matters.
func V7UnixMillis(id uuid.UUID) (uint64, error) {
	return v7UnixMillis(id), nil
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
	out, rc := newV7(unixMillis)
	switch rc {
	case 0:
		return out, nil
	case 2:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v7 failed with code %d: %w", rc, ErrTimestampOutOfRange)
	default:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v7 failed with code %d: %w", rc, ErrRandomSource)
	}
}

// NewV7AtTime creates a time-sortable UUID version 7 (RFC 9562 §6.2) from a time.Time — pulls
// the Unix-epoch milliseconds off t and mints it through NewV7At.
func NewV7AtTime(t time.Time) (uuid.UUID, error) {
	return NewV7At(uint64(t.UnixMilli()))
}

// GetTimestamp recovers the UTC timestamp embedded in id as a time.Time, or ErrNotTimeBased if
// id isn't an RFC 9562 version 7 UUID. Unlike V7Timestamp, this checks the variant and version
// itself, so a caller doesn't need to already know (or separately check) what id is before
// asking: a 7 nibble under a variant other than RFC 9562's has no version, and so no
// timestamp. Only version 7 has one here: a version 6 or version 1 UUID from another
// generator returns ErrNotTimeBased too. One core call does the check and the read, with no
// bit-layout logic here. GetTimestampIn is the form for a SQL-ordered value.
func GetTimestamp(id uuid.UUID) (time.Time, error) {
	return timestampIn(id, uint32(LayoutRfc9562))
}

// MaxV7Batch is the most UUIDs one version 7 batch (NewV7Batch, NewV7BatchAt and the FillV7
// family) mints: 67,108,864, the size of the 26-bit counter that orders UUIDs within a
// millisecond.
//
// Every batch up to this size is in strictly increasing order. The counter is one
// process-wide sequence, so a batch can straddle the point where it wraps back to 0; the
// UUIDs from there on carry a timestamp one millisecond later than the one supplied rather
// than sorting before the ones ahead of them. A larger batch would have to reuse counter
// values within one millisecond, so it is refused with ErrBatchTooLarge before anything is
// allocated or written.
//
// The roll-forward orders one batch, not the stream. The next batch or NewV7At call in the
// same real millisecond starts its counter just past the wrap and carries the supplied
// timestamp, so it sorts before the previous batch's tail, stamped a millisecond later; two
// single calls either side of the wrap in one millisecond sort in reverse the same way. It
// happens at most once per MaxV7Batch UUIDs the process mints.
const MaxV7Batch = 1 << 26

// errV7BatchTooLarge is the ErrBatchTooLarge a v7 batch of count returns.
func errV7BatchTooLarge(count int) error {
	return fmt.Errorf("%w: got %d", ErrBatchTooLarge, count)
}

// NewV7Batch creates count time-sortable version 7 UUIDs sharing one timestamp capture and
// one contiguous block of the monotonic counter, using the current time — one native call and
// one random-bytes fetch instead of count of each. Same order and limits as NewV7BatchAt.
func NewV7Batch(count int) ([]uuid.UUID, error) {
	return NewV7BatchAt(count, uint64(time.Now().UnixMilli()))
}

// NewV7BatchAt creates count time-sortable version 7 UUIDs sharing one unixMillis timestamp
// capture and one contiguous block of the monotonic counter, in strictly increasing order: if
// the batch straddles the counter's wrap, the UUIDs from the wrap on carry unixMillis + 1 (see
// MaxV7Batch, which also covers how a following batch orders against them). A count of 0
// returns a nil slice; a negative one returns ErrNegativeCount, and one past MaxV7Batch
// ErrBatchTooLarge, before the result is allocated.
func NewV7BatchAt(count int, unixMillis uint64) ([]uuid.UUID, error) {
	if count < 0 {
		return nil, fmt.Errorf("%w: got %d", ErrNegativeCount, count)
	}
	if count > MaxV7Batch {
		return nil, errV7BatchTooLarge(count)
	}
	if count == 0 {
		return nil, nil
	}
	// Delegates to FillV7At: because a []uuid.UUID is already contiguous 16-byte records,
	// the native call writes straight into result. No scratch byte buffer and no
	// per-element copy.
	result := make([]uuid.UUID, count)
	if err := FillV7At(result, unixMillis); err != nil {
		return nil, err
	}
	return result, nil
}

// errSqlOrder maps a SQL-order conversion's return code onto this package's errors. Each is
// a prebuilt sentinel, so a refusal allocates nothing either.
func errSqlOrder(rc int32) error {
	switch rc {
	case 0:
		return nil
	case 5:
		return ErrWrongVersion
	default:
		return fmt.Errorf("hyperuuid: SQL-order conversion failed with code %d", rc)
	}
}

// sqlOrder runs one SQL-order conversion, returning the zero UUID with the error on a refusal.
func sqlOrder(id uuid.UUID, call func(uuid.UUID) (uuid.UUID, int32)) (uuid.UUID, error) {
	out, rc := call(id)
	if err := errSqlOrder(rc); err != nil {
		return uuid.UUID{}, err
	}
	return out, nil
}

// V7ToSqlOrder converts an RFC 9562 version 7 id to SQL Server order: a version 8 UUID (RFC
// 9562 §5.8, the version for custom layouts) whose uniqueidentifier sort is creation order.
//
// System.Data.SqlTypes.SqlGuid comparison — and therefore T-SQL ORDER BY on a
// uniqueidentifier column — doesn't compare a GUID's 16 bytes left to right; it uses a fixed
// significance order (most significant first: octets 10,11,12,13,14,15, 8,9, 6,7, 4,5,
// 0,1,2,3). The layout puts the version nibble 8 at octet 7's top nibble and the RFC variant
// at octet 8's top two bits — where SQL Server's text form, and a .NET Guid, show them — and
// fills the other 122 bits, most significant first, with all 122 of the version 7's own: the
// 48-bit timestamp, the 26-bit counter, then the 48 trailing random bits as one block. So the
// timestamp lands in octets 10-15, the counter split around the version and variant, and the
// random bits in 4-5 and 0-3, and values sort by timestamp, then by counter within a
// millisecond. VersionIn with LayoutSqlServer reads 7 back from the 8 and the variant. Every
// version 7 converts, whatever its timestamp. The conversion is computed in the native core and verified there (and
// independently, against the real SqlGuid comparator, in this project's C# test suite).
//
// The result is no longer a version 7: RFC 9562 §6.11 sends a byte order other than the
// RFC's to version 8. V7FromSqlOrder gives back the exact original.
//
// It refuses a value that isn't an RFC 9562 version 7 with ErrWrongVersion and returns the
// zero UUID. The check reads the bytes as RFC order, so a value already in SQL order is
// converted a second time about one time in 16, when it happens to carry a 7 and the RFC
// variant there: keep track of which order a value is in.
func V7ToSqlOrder(id uuid.UUID) (uuid.UUID, error) {
	return sqlOrder(id, v7ToSqlOrder)
}

// V7FromSqlOrder is the inverse of V7ToSqlOrder — converts a SQL-ordered value back to the
// RFC 9562 version 7 it came from. Anything VersionIn with LayoutSqlServer doesn't read as a
// converted version 7 — an RFC-ordered value, say — returns ErrWrongVersion and the zero
// UUID.
func V7FromSqlOrder(id uuid.UUID) (uuid.UUID, error) {
	return sqlOrder(id, v7ToRfcOrder)
}

// ---- Destination-buffer fills -------------------------------------------------------
//
// NewV7Batch above allocates a fresh slice per call. These write into one the
// caller already owns, which is the shape that lets a hot path reuse a buffer instead of
// handing the collector a new one every batch — the same reason io.Reader takes a
// destination rather than returning a slice.
//
// A []uuid.UUID needs no intermediate byte buffer and no per-element conversion here:
// uuid.UUID is [16]byte and a Go slice of it is contiguous 16-byte records with no padding
// (asserted at the top of the fill, not assumed), which is exactly the layout the native
// core writes. So the whole batch lands in the caller's slice in one native call. The C#
// binding cannot do this — System.Guid's in-memory layout is mixed-endian and isn't RFC byte
// order, so it has to convert every element — which is why the byte-slice variants below
// exist there for performance but exist here only for callers who genuinely want raw bytes
// (a wire buffer, a database parameter) rather than uuid.UUID values.

// errBatch maps a native batch return code onto this package's sentinel errors.
func errBatch(fn string, rc int32) error {
	switch rc {
	case 0:
		return nil
	case 2:
		return fmt.Errorf("%s failed with code %d: %w", fn, rc, ErrTimestampOutOfRange)
	case 3:
		return fmt.Errorf("%s failed with code %d: %w", fn, rc, ErrBatchUnaddressable)
	case 4:
		return fmt.Errorf("%s failed with code %d: %w", fn, rc, ErrBatchTooLarge)
	default:
		return fmt.Errorf("%s failed with code %d: %w", fn, rc, ErrRandomSource)
	}
}

// fillUUIDs writes len(dst) UUIDs straight into dst's backing array.
func fillUUIDs(dst []uuid.UUID, unixMillis uint64, fn string,
	call func(uint64, uint32, unsafe.Pointer) int32) error {
	if len(dst) == 0 {
		return nil
	}
	if uint64(len(dst)) > math.MaxUint32 {
		return fmt.Errorf("hyperuuid: batch of %d exceeds the native count limit", len(dst))
	}
	// uuid.UUID is [16]byte, so &dst[0] addresses one contiguous len(dst)*16 byte region.
	return errBatch(fn, call(unixMillis, uint32(len(dst)), unsafe.Pointer(&dst[0])))
}

// fillBytes writes len(dst)/16 UUIDs straight into dst, which must be a whole number of them.
func fillBytes(dst []byte, unixMillis uint64, fn string,
	call func(uint64, uint32, unsafe.Pointer) int32) error {
	if len(dst)%16 != 0 {
		return fmt.Errorf("%w: got %d", ErrBufferNotWholeUUIDs, len(dst))
	}
	if len(dst) == 0 {
		return nil
	}
	count := len(dst) / 16
	if uint64(count) > math.MaxUint32 {
		return fmt.Errorf("hyperuuid: batch of %d exceeds the native count limit", count)
	}
	return errBatch(fn, call(unixMillis, uint32(count), unsafe.Pointer(&dst[0])))
}

// FillV7 fills dst with time-sortable version 7 UUIDs sharing one timestamp capture and one
// contiguous block of the monotonic counter, using the current time. Same order and limits
// as FillV7At.
func FillV7(dst []uuid.UUID) error {
	return FillV7At(dst, uint64(time.Now().UnixMilli()))
}

// FillV7At fills dst with version 7 UUIDs sharing the given unixMillis timestamp capture and
// one contiguous block of the monotonic counter, in strictly increasing order: if the batch
// straddles the counter's wrap, the UUIDs from the wrap on carry unixMillis + 1 (see
// MaxV7Batch, which also covers how a following batch orders against them). A dst longer than
// MaxV7Batch returns ErrBatchTooLarge with nothing written.
func FillV7At(dst []uuid.UUID, unixMillis uint64) error {
	if len(dst) > MaxV7Batch {
		return errV7BatchTooLarge(len(dst))
	}
	return fillUUIDs(dst, unixMillis, "uuid_new_v7_batch",
		newV7Batch)
}

// FillV7Bytes fills dst with raw RFC 9562-ordered version 7 UUID bytes, 16 per UUID, using
// the current time. len(dst) must be a multiple of 16; anything else returns
// ErrBufferNotWholeUUIDs.
func FillV7Bytes(dst []byte) error {
	return FillV7BytesAt(dst, uint64(time.Now().UnixMilli()))
}

// FillV7BytesAt fills dst with raw RFC 9562-ordered version 7 UUID bytes sharing the given
// unixMillis timestamp capture and one contiguous block of the monotonic counter, in strictly
// increasing order (see FillV7At). A dst of more than MaxV7Batch UUIDs returns
// ErrBatchTooLarge with nothing written.
func FillV7BytesAt(dst []byte, unixMillis uint64) error {
	if len(dst)%16 == 0 && len(dst)/16 > MaxV7Batch {
		return errV7BatchTooLarge(len(dst) / 16)
	}
	return fillBytes(dst, unixMillis, "uuid_new_v7_batch",
		newV7Batch)
}

// ---- Raw-byte SQL-order transforms --------------------------------------------------
//
// The same native conversions as V7ToSqlOrder and V7FromSqlOrder above, with the same refusals,
// rewriting a caller's own 16-byte buffer in place instead of taking and returning a
// uuid.UUID. A refusal leaves the buffer as it was. Useful when the value
// is already bytes on its way to a wire format or a database parameter, and — because they
// are pure byte-in/byte-out — they are the form a byte-level correctness oracle can be
// pointed at directly, shared across every binding in this repo rather than re-expressed in
// each language's own UUID type.

func sqlOrderBytes(b []byte, call func(unsafe.Pointer) int32) error {
	if len(b) != 16 {
		return fmt.Errorf("%w: got %d", ErrNotOneUUID, len(b))
	}
	return errSqlOrder(call(unsafe.Pointer(&b[0])))
}

// V7ToSqlOrderBytes rewrites the 16 RFC 9562-ordered version 7 bytes in b into SQL Server
// order, in place. See V7ToSqlOrder for the layout and the refusals.
func V7ToSqlOrderBytes(b []byte) error {
	return sqlOrderBytes(b, v7ToSqlOrderBytes)
}

// V7FromSqlOrderBytes is the inverse of V7ToSqlOrderBytes, in place.
func V7FromSqlOrderBytes(b []byte) error {
	return sqlOrderBytes(b, v7ToRfcOrderBytes)
}
