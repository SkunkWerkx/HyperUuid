package hyperuuid

import "errors"

// ErrRandomSource is returned when the native random source fails (uuid_new_v4/v5/v7 return
// code 1).
var ErrRandomSource = errors.New("hyperuuid: random source failure")

// ErrTimestampOutOfRange is returned by NewV7At when unixMillis doesn't fit the RFC 9562
// 48-bit unix_ts_ms field (uuid_new_v7 return code 2).
var ErrTimestampOutOfRange = errors.New("hyperuuid: unix millisecond timestamp must fit within 48 bits")

// ErrNotTimeBased is returned by GetTimestamp when the given UUID isn't version 6 or 7.
var ErrNotTimeBased = errors.New("hyperuuid: uuid is not a version 6 or 7 uuid")

// ErrBufferNotWholeUUIDs is returned by the FillV6Bytes/FillV7Bytes family when the
// destination's length isn't a multiple of 16 — one whole UUID per 16 bytes.
var ErrBufferNotWholeUUIDs = errors.New("hyperuuid: destination length must be a multiple of 16")

// ErrNotOneUUID is returned by the raw-byte SQL-order transforms when the buffer isn't
// exactly 16 bytes.
var ErrNotOneUUID = errors.New("hyperuuid: buffer must be exactly 16 bytes")
