// Package hyperuuid provides RFC 9562 UUID generation (v4 random, v5 deterministic, v6/v7
// time-sortable) by calling the Rust core, linked into the binary as a static library through
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
	out, rc := newV6(unixMillis)
	switch rc {
	case 0:
		return out, nil
	case 2:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v6 failed with code %d: %w", rc, ErrTimestampOutOfRange)
	default:
		return uuid.UUID{}, fmt.Errorf("uuid_new_v6 failed with code %d: %w", rc, ErrRandomSource)
	}
}

// NewV6AtTime creates a time-sortable UUID version 6 (RFC 9562 §5.6) from a time.Time — pulls
// the Unix-epoch milliseconds off t and mints it through NewV6At.
func NewV6AtTime(t time.Time) (uuid.UUID, error) {
	return NewV6At(uint64(t.UnixMilli()))
}

// NewV6Batch creates count time-sortable version 6 UUIDs sharing one timestamp capture,
// using the current time — one native call and one random-bytes fetch instead of count of
// each.
func NewV6Batch(count int) ([]uuid.UUID, error) {
	return NewV6BatchAt(count, uint64(time.Now().UnixMilli()))
}

// NewV6BatchAt creates count time-sortable version 6 UUIDs sharing one unixMillis timestamp
// capture. clock_seq and node are independently random per item — unlike version 7, there is
// no monotonic counter, so items are not guaranteed to sort in creation order. A count of 0
// returns a nil slice; a negative one returns ErrNegativeCount.
func NewV6BatchAt(count int, unixMillis uint64) ([]uuid.UUID, error) {
	if count < 0 {
		return nil, fmt.Errorf("%w: got %d", ErrNegativeCount, count)
	}
	if count == 0 {
		return nil, nil
	}
	// Delegates to FillV6At: because a []uuid.UUID is already contiguous 16-byte records,
	// the native call writes straight into result. No scratch byte buffer and no
	// per-element copy — this used to allocate both and then copy between them.
	result := make([]uuid.UUID, count)
	if err := FillV6At(result, unixMillis); err != nil {
		return nil, err
	}
	return result, nil
}

// V6UnixMillis recovers the Unix-epoch millisecond timestamp embedded in a version 6 UUID's
// timestamp field. Only meaningful when id.Version() == 6 — the RFC 9562 bit layout doesn't
// distinguish "not a v6 UUID" from "v6 UUID with a very early timestamp", so the caller is
// responsible for checking that first if it matters.
func V6UnixMillis(id uuid.UUID) (uint64, error) {
	return v6UnixMillis(id), nil
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
// id isn't an RFC 9562 version 6 or 7 UUID. Unlike V6Timestamp/V7Timestamp, this checks the
// variant and version itself, so a caller doesn't need to already know (or separately check)
// which version id is before asking: a 6 or 7 nibble under a variant other than RFC 9562's
// has no version, and so no timestamp. One core call does the check and the read, with no
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
// allocated or written. Version 6 has no counter and no such limit.
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
// MaxV7Batch, which also covers how a following batch orders against them). A count of 0 returns a nil slice; a negative one returns ErrNegativeCount, and
// one past MaxV7Batch ErrBatchTooLarge, before the result is allocated.
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
	// per-element copy — this used to allocate both and then copy between them.
	result := make([]uuid.UUID, count)
	if err := FillV7At(result, unixMillis); err != nil {
		return nil, err
	}
	return result, nil
}

// V7ToSqlOrder converts an RFC 9562-ordered version 7 id to the byte order SQL Server's
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
// reimplementing the math. Meaningful only for a genuine version 7 UUID — see V6ToSqlOrder
// for the version 6 equivalent.
func V7ToSqlOrder(id uuid.UUID) (uuid.UUID, error) {
	return v7ToSqlOrder(id), nil
}

// V7FromSqlOrder is the inverse of V7ToSqlOrder — converts a SQL-Server-ordered version 7 id
// back to RFC 9562 order.
func V7FromSqlOrder(id uuid.UUID) (uuid.UUID, error) {
	return v7ToRfcOrder(id), nil
}

// V6ToSqlOrder converts an RFC 9562-ordered version 6 id to the byte order SQL Server's
// uniqueidentifier needs on the wire to sort by creation order.
//
// Same SqlGuid significance order as V7ToSqlOrder, applied to v6's very different field
// layout. v6 has no monotonic counter the way v7 does; the only field that determines its
// creation order is the 60-bit timestamp itself, so this moves that whole timestamp — most
// significant chunk first — into the comparison's most significant octets, and relocates
// clock_seq/node (no ordering value here — generated randomly on every call, not a counter)
// into the remaining octets. Version and variant end up at different byte offsets than
// V7ToSqlOrder's result (octet 8's top nibble and octet 6's top two bits here, not 7/8) —
// fine, since the two versions are separate functions and a caller always knows which one
// it's calling.
//
// Unlike v7, two version 6 UUIDs minted at the same millisecond have identical timestamp
// bits — clock_seq/node are independently random, not a counter — so this doesn't (and
// can't) make same-millisecond v6 UUIDs sort in creation order any more than plain RFC order
// already does. Distinct timestamps sort correctly; same-timestamp ties don't, by the RFC's
// own v6 design, not a limitation introduced here.
//
// Meaningful only for a genuine version 6 UUID.
func V6ToSqlOrder(id uuid.UUID) (uuid.UUID, error) {
	return v6ToSqlOrder(id), nil
}

// V6FromSqlOrder is the inverse of V6ToSqlOrder — converts a SQL-Server-ordered version 6 id
// back to RFC 9562 order.
func V6FromSqlOrder(id uuid.UUID) (uuid.UUID, error) {
	return v6ToRfcOrder(id), nil
}

// ---- Destination-buffer fills -------------------------------------------------------
//
// NewV6Batch/NewV7Batch above allocate a fresh slice per call. These write into one the
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

// FillV6 fills dst with time-sortable version 6 UUIDs sharing one timestamp capture, using
// the current time. clock_seq and node are independently random per item — unlike version 7
// there is no monotonic counter, so items are not guaranteed to sort in creation order.
func FillV6(dst []uuid.UUID) error {
	return FillV6At(dst, uint64(time.Now().UnixMilli()))
}

// FillV6At fills dst with version 6 UUIDs sharing the given unixMillis timestamp capture.
func FillV6At(dst []uuid.UUID, unixMillis uint64) error {
	return fillUUIDs(dst, unixMillis, "uuid_new_v6_batch",
		newV6Batch)
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
// MaxV7Batch, which also covers how a following batch orders against them). A dst longer than MaxV7Batch returns ErrBatchTooLarge with nothing written.
func FillV7At(dst []uuid.UUID, unixMillis uint64) error {
	if len(dst) > MaxV7Batch {
		return errV7BatchTooLarge(len(dst))
	}
	return fillUUIDs(dst, unixMillis, "uuid_new_v7_batch",
		newV7Batch)
}

// FillV6Bytes fills dst with raw RFC 9562-ordered version 6 UUID bytes, 16 per UUID, using
// the current time. len(dst) must be a multiple of 16; anything else returns
// ErrBufferNotWholeUUIDs.
func FillV6Bytes(dst []byte) error {
	return FillV6BytesAt(dst, uint64(time.Now().UnixMilli()))
}

// FillV6BytesAt fills dst with raw RFC 9562-ordered version 6 UUID bytes sharing the given
// unixMillis timestamp capture.
func FillV6BytesAt(dst []byte, unixMillis uint64) error {
	return fillBytes(dst, unixMillis, "uuid_new_v6_batch",
		newV6Batch)
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
// The same native permutations as V6/V7To/FromSqlOrder above, rewriting a caller's own
// 16-byte buffer in place instead of taking and returning a uuid.UUID. Useful when the value
// is already bytes on its way to a wire format or a database parameter, and — because they
// are pure byte-in/byte-out — they are the form a byte-level correctness oracle can be
// pointed at directly, shared across every binding in this repo rather than re-expressed in
// each language's own UUID type.

func sqlOrderBytes(b []byte, call func(unsafe.Pointer)) error {
	if len(b) != 16 {
		return fmt.Errorf("%w: got %d", ErrNotOneUUID, len(b))
	}
	call(unsafe.Pointer(&b[0]))
	return nil
}

// V7ToSqlOrderBytes rewrites the 16 RFC 9562-ordered version 7 bytes in b into SQL Server
// uniqueidentifier sort order, in place. See V7ToSqlOrder for the byte-level rationale.
func V7ToSqlOrderBytes(b []byte) error {
	return sqlOrderBytes(b, v7ToSqlOrderBytes)
}

// V7FromSqlOrderBytes is the inverse of V7ToSqlOrderBytes, in place.
func V7FromSqlOrderBytes(b []byte) error {
	return sqlOrderBytes(b, v7ToRfcOrderBytes)
}

// V6ToSqlOrderBytes rewrites the 16 RFC 9562-ordered version 6 bytes in b into SQL Server
// uniqueidentifier sort order, in place. See V6ToSqlOrder for the byte-level rationale.
func V6ToSqlOrderBytes(b []byte) error {
	return sqlOrderBytes(b, v6ToSqlOrderBytes)
}

// V6FromSqlOrderBytes is the inverse of V6ToSqlOrderBytes, in place.
func V6FromSqlOrderBytes(b []byte) error {
	return sqlOrderBytes(b, v6ToRfcOrderBytes)
}
