package hyperuuid

// ---- Inspection and layout-aware timestamps ------------------------------------------
//
// The layout knowledge lives in the core: these only hand it a UUID's 16 bytes and say which
// order they are in. A uuid.UUID is its bytes, so there is no conversion step in either
// layout — a value from V7ToSqlOrder or V6ToSqlOrder already holds SQL Server's
// uniqueidentifier wire bytes, which is what LayoutSqlServer means.

import (
	"fmt"
	"time"

	"github.com/google/uuid"
)

// UuidLayout is the byte order a UUID is held in, for the inspection and timestamp functions
// that take one. The values are the core's own codes, with 0 reserved, so a zero value left
// unset is LayoutUnspecified and is refused with ErrInvalidLayout rather than taken as a
// guess.
type UuidLayout uint32

const (
	// LayoutUnspecified is the zero value, refused by every function that takes a layout.
	LayoutUnspecified UuidLayout = 0
	// LayoutRfc9562 is RFC 9562 network order: what every other function in this package,
	// and google/uuid, takes and returns.
	LayoutRfc9562 UuidLayout = 1
	// LayoutSqlServer is the order V7ToSqlOrder and V6ToSqlOrder return, which SQL Server's
	// uniqueidentifier sorts by creation order. Defined for versions 6 and 7 only.
	LayoutSqlServer UuidLayout = 2
)

// String returns the layout's constant name.
func (l UuidLayout) String() string {
	switch l {
	case LayoutUnspecified:
		return "LayoutUnspecified"
	case LayoutRfc9562:
		return "LayoutRfc9562"
	case LayoutSqlServer:
		return "LayoutSqlServer"
	default:
		return fmt.Sprintf("UuidLayout(%d)", uint32(l))
	}
}

// UuidVariant is the variant field of a UUID (RFC 9562 §4.1), which says how the rest of its
// bits are laid out; see Variant. Only VariantRfc9562 has versions. The values are the
// core's own codes, with 0 reserved for VariantUnspecified, which Variant never returns.
//
// It is not google/uuid's uuid.Variant: that type has no Unspecified value and names the RFC
// variant after RFC 4122.
type UuidVariant uint32

const (
	// VariantUnspecified is the zero value, never returned by Variant.
	VariantUnspecified UuidVariant = 0
	// VariantNcs is 0xxx: reserved, Network Computing System backward compatibility.
	// Includes Nil.
	VariantNcs UuidVariant = 1
	// VariantRfc9562 is 10xx: the variant RFC 9562 (and RFC 4122 before it) specifies.
	VariantRfc9562 UuidVariant = 2
	// VariantMicrosoft is 110x: reserved, Microsoft Corporation backward compatibility.
	VariantMicrosoft UuidVariant = 3
	// VariantFuture is 111x: reserved for future definition. Includes Max.
	VariantFuture UuidVariant = 4
)

// String returns the variant's constant name.
func (v UuidVariant) String() string {
	switch v {
	case VariantUnspecified:
		return "VariantUnspecified"
	case VariantNcs:
		return "VariantNcs"
	case VariantRfc9562:
		return "VariantRfc9562"
	case VariantMicrosoft:
		return "VariantMicrosoft"
	case VariantFuture:
		return "VariantFuture"
	default:
		return fmt.Sprintf("UuidVariant(%d)", uint32(v))
	}
}

// layoutCode returns the core's code for layout, or ErrInvalidLayout for anything else.
func layoutCode(layout UuidLayout) (uint32, error) {
	switch layout {
	case LayoutRfc9562, LayoutSqlServer:
		return uint32(layout), nil
	default:
		return 0, fmt.Errorf("%w: got %v", ErrInvalidLayout, layout)
	}
}

// oneUUID copies a 16-byte buffer into a uuid.UUID, or returns ErrNotOneUUID.
func oneUUID(b []byte) (uuid.UUID, error) {
	var id uuid.UUID
	if len(b) != 16 {
		return id, fmt.Errorf("%w: got %d", ErrNotOneUUID, len(b))
	}
	copy(id[:], b)
	return id, nil
}

// Version returns the RFC 9562 version nibble of id, 0 through 15 — 0 for Nil, 15 for Max.
// It says nothing about the variant: use IsRfc when the question is "an RFC 9562 UUID of
// version N". The same answer as google/uuid's id.Version(), read by the core.
//
// It reads id as RFC 9562 order. For a value from V7ToSqlOrder or V6ToSqlOrder, or read back
// from a uniqueidentifier column, use VersionIn with LayoutSqlServer: Version reads the wrong
// bytes there.
func Version(id uuid.UUID) int {
	return int(version(id, uint32(LayoutRfc9562)))
}

// VersionIn returns the version of an id held in layout's byte order. In LayoutRfc9562 it is
// Version. LayoutSqlServer is defined only for the two versions that have a SQL Server order,
// and answers 6 or 7 when the bytes form a SQL-ordered version 6 or 7 RFC 9562 UUID, and 0
// when they don't. The version nibble lands at a different byte for each, and the other
// version's random bits can mimic it there, so the core checks the variant bits too, where
// each version puts them, and the answer never confuses the two. The layout itself is the
// caller's to know: 16 bytes carry no mark of the order they are in, and an RFC-ordered value
// can happen to form a valid SQL-ordered version 7 (about one random version 4 in 16 does),
// so pass LayoutSqlServer at every call site that can see a SQL-ordered value. An undefined
// layout returns ErrInvalidLayout.
func VersionIn(id uuid.UUID, layout UuidLayout) (int, error) {
	code, err := layoutCode(layout)
	if err != nil {
		return 0, err
	}
	return int(version(id, code)), nil
}

// VersionBytes is VersionIn over 16 raw bytes already in layout's order. A buffer that isn't
// exactly 16 bytes returns ErrNotOneUUID.
func VersionBytes(b []byte, layout UuidLayout) (int, error) {
	code, err := layoutCode(layout)
	if err != nil {
		return 0, err
	}
	id, err := oneUUID(b)
	if err != nil {
		return 0, err
	}
	return int(version(id, code)), nil
}

// Variant returns the variant field of an RFC 9562-ordered id (RFC 9562 §4.1): VariantNcs
// for Nil, VariantFuture for Max, and VariantRfc9562 for anything this package or google/uuid
// mints. Never VariantUnspecified.
//
// It reads RFC 9562 order only, and there is no layout form: in SQL Server order the variant
// sits at a different byte for each version. Don't feed it a uniqueidentifier read-back or a
// V7ToSqlOrder result; to validate one of those, use IsRfcIn with LayoutSqlServer, which
// checks the variant where that version puts it.
func Variant(id uuid.UUID) UuidVariant {
	return UuidVariant(variant(id))
}

// VariantBytes is Variant over 16 raw RFC 9562-ordered bytes, with the same RFC-order-only
// caveat. A buffer that isn't exactly 16 bytes returns ErrNotOneUUID.
func VariantBytes(b []byte) (UuidVariant, error) {
	id, err := oneUUID(b)
	if err != nil {
		return VariantUnspecified, err
	}
	return UuidVariant(variant(id)), nil
}

// rfcVersion narrows version for the core, which takes it as a uint32: anything outside the
// nibble is reported as 16, which never matches, rather than truncated onto one that might.
func rfcVersion(version int) uint32 {
	if version < 0 || version > 15 {
		return 16
	}
	return uint32(version)
}

// IsRfc reports whether id is an RFC 9562 UUID of the given version — the RFC variant and that
// version nibble, in one native call. It is the guard to run before trusting a value's
// version-specific fields, such as a version 7's timestamp. A version outside 0-15 is simply
// never matched.
//
// It reads id as RFC 9562 order. A SQL-ordered value — from V7ToSqlOrder, or read back from a
// uniqueidentifier column — needs IsRfcIn with LayoutSqlServer: IsRfc reads the wrong bytes
// there and answers for whatever they happen to hold. There is deliberately no
// layout-agnostic form: the caller holding the value knows its order, and only the layout it
// names says which bytes to read.
func IsRfc(id uuid.UUID, version int) bool {
	return isRfc(id, rfcVersion(version), uint32(LayoutRfc9562))
}

// IsRfcIn is IsRfc for an id held in layout's byte order — in LayoutSqlServer, only versions
// 6 and 7 can be true (see VersionIn), and the variant is checked where that version puts it,
// which makes this the guard for a SQL-ordered value. An undefined layout returns
// ErrInvalidLayout.
func IsRfcIn(id uuid.UUID, version int, layout UuidLayout) (bool, error) {
	code, err := layoutCode(layout)
	if err != nil {
		return false, err
	}
	return isRfc(id, rfcVersion(version), code), nil
}

// IsRfcBytes is IsRfcIn over 16 raw bytes already in layout's order. A buffer that isn't
// exactly 16 bytes returns ErrNotOneUUID.
func IsRfcBytes(b []byte, version int, layout UuidLayout) (bool, error) {
	code, err := layoutCode(layout)
	if err != nil {
		return false, err
	}
	id, err := oneUUID(b)
	if err != nil {
		return false, err
	}
	return isRfc(id, rfcVersion(version), code), nil
}

// V6UnixMillisIn is V6UnixMillis for an id held in layout's byte order, reading a SQL-ordered
// value's permuted bytes directly, with no V6FromSqlOrder first. Meaningful only for a genuine
// version 6 UUID in that layout; IsRfcIn is the check. An undefined layout returns
// ErrInvalidLayout.
func V6UnixMillisIn(id uuid.UUID, layout UuidLayout) (uint64, error) {
	code, err := layoutCode(layout)
	if err != nil {
		return 0, err
	}
	return v6UnixMillisIn(id, code), nil
}

// V6TimestampIn is V6Timestamp for an id held in layout's byte order; see V6UnixMillisIn.
func V6TimestampIn(id uuid.UUID, layout UuidLayout) (time.Time, error) {
	millis, err := V6UnixMillisIn(id, layout)
	if err != nil {
		return time.Time{}, err
	}
	return time.UnixMilli(int64(millis)).UTC(), nil
}

// V7UnixMillisIn is V7UnixMillis for an id held in layout's byte order, reading a SQL-ordered
// value's permuted bytes directly, with no V7FromSqlOrder first. Meaningful only for a genuine
// version 7 UUID in that layout; IsRfcIn is the check. An undefined layout returns
// ErrInvalidLayout.
func V7UnixMillisIn(id uuid.UUID, layout UuidLayout) (uint64, error) {
	code, err := layoutCode(layout)
	if err != nil {
		return 0, err
	}
	return v7UnixMillisIn(id, code), nil
}

// V7TimestampIn is V7Timestamp for an id held in layout's byte order; see V7UnixMillisIn.
func V7TimestampIn(id uuid.UUID, layout UuidLayout) (time.Time, error) {
	millis, err := V7UnixMillisIn(id, layout)
	if err != nil {
		return time.Time{}, err
	}
	return time.UnixMilli(int64(millis)).UTC(), nil
}

// GetTimestampIn is GetTimestamp for an id held in layout's byte order — in LayoutSqlServer,
// the timestamp of a value straight from V7ToSqlOrder or V6ToSqlOrder (or a uniqueidentifier
// column), read from its permuted bytes with no conversion back first. It returns
// ErrNotTimeBased for anything that isn't an RFC 9562 version 6 or 7 UUID in that layout —
// the variant is checked as well as the version, so a 6 or 7 nibble under another variant
// has no timestamp — and ErrInvalidLayout for an undefined layout.
func GetTimestampIn(id uuid.UUID, layout UuidLayout) (time.Time, error) {
	code, err := layoutCode(layout)
	if err != nil {
		return time.Time{}, err
	}
	return timestampIn(id, code)
}

// timestampIn is GetTimestampIn past the layout check: one core call checks the variant and
// version and reads the timestamp.
func timestampIn(id uuid.UUID, code uint32) (time.Time, error) {
	if version, millis := getTimestamp(id, code); version != 0 {
		return time.UnixMilli(int64(millis)).UTC(), nil
	}
	return time.Time{}, ErrNotTimeBased
}
