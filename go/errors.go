package hyperuuid

import "errors"

// ErrNativeUnavailable is never returned: the core is linked into the binary, so there is no
// load that can fail.
//
// Deprecated: kept only so code that tests for it keeps compiling.
var ErrNativeUnavailable = errors.New("hyperuuid: native library unavailable")

// ErrRandomSource is returned when the native random source fails (return code 1 from
// uuid_new_v4, uuid_new_v6, uuid_new_v7 or either batch function). Version 5 draws no
// entropy, so NewV5 never returns it.
var ErrRandomSource = errors.New("hyperuuid: random source failure")

// ErrTimestampOutOfRange is returned by every version 6 and version 7 generator — NewV6At,
// NewV7At, their batch and Fill forms — when unixMillis doesn't fit the version's own
// timestamp field (return code 2). The two limits differ: version 7 holds 48 bits of Unix
// milliseconds, version 6 a 60-bit count of 100 ns ticks since 1582-10-15, which runs out
// earlier (in the year 5236).
var ErrTimestampOutOfRange = errors.New("hyperuuid: unix millisecond timestamp out of range for this UUID version")

// ErrNotTimeBased is returned by GetTimestamp and GetTimestampIn when the given UUID isn't an
// RFC 9562 version 6 or 7 UUID (in that layout): another version, or another variant.
var ErrNotTimeBased = errors.New("hyperuuid: uuid is not a version 6 or 7 uuid")

// ErrNegativeCount is returned by NewV6Batch/NewV7Batch and their At forms when count is
// negative.
var ErrNegativeCount = errors.New("hyperuuid: batch count must not be negative")

// ErrBufferNotWholeUUIDs is returned by the FillV6Bytes/FillV7Bytes family when the
// destination's length isn't a multiple of 16 — one whole UUID per 16 bytes.
var ErrBufferNotWholeUUIDs = errors.New("hyperuuid: destination length must be a multiple of 16")

// ErrNotOneUUID is returned by the raw-byte SQL-order transforms when the buffer isn't
// exactly 16 bytes.
var ErrNotOneUUID = errors.New("hyperuuid: buffer must be exactly 16 bytes")

// ErrInvalidLayout is returned by every function that takes a UuidLayout when it is neither
// LayoutRfc9562 nor LayoutSqlServer — LayoutUnspecified (a zero value left unset) included.
// The layout is never guessed and never passed through to the core.
var ErrInvalidLayout = errors.New("hyperuuid: layout must be LayoutRfc9562 or LayoutSqlServer")

// ErrBatchTooLarge is returned by NewV7Batch, NewV7BatchAt and the FillV7 family when one
// batch asks for more than MaxV7Batch UUIDs (return code 4 from uuid_new_v7_batch, though the
// count is checked here first, before anything is allocated). Version 6 has no such limit.
var ErrBatchTooLarge = errors.New("hyperuuid: a single version 7 batch takes at most 67108864 UUIDs (the 26-bit counter space)")

// ErrBatchUnaddressable is returned by a batch or fill when the core cannot address count*16
// bytes on this platform (return code 3), which only a 32-bit target — WebAssembly under
// TinyGo — could reach, and only for version 6, whose batches have no MaxV7Batch.
var ErrBatchUnaddressable = errors.New("hyperuuid: batch is too large to address on this platform")
