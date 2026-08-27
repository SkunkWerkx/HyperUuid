package hyperuuid

import "errors"

// ErrRandomSource is returned when the native random source fails (uuid_new_v4/v5/v7 return
// code 1).
var ErrRandomSource = errors.New("hyperuuid: random source failure")

// ErrTimestampOutOfRange is returned by NewV7At when unixMillis doesn't fit the RFC 9562
// 48-bit unix_ts_ms field (uuid_new_v7 return code 2).
var ErrTimestampOutOfRange = errors.New("hyperuuid: unix millisecond timestamp must fit within 48 bits")
