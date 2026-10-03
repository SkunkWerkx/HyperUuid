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

// ErrNotTimeBased is returned by GetTimestamp when the given UUID isn't version 6 or 7.
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
