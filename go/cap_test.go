package hyperuuid

import (
	"errors"
	"testing"
	"time"

	"github.com/google/uuid"
)

// The generation cap and its guard on the time.Time door, both directions.
func TestNewV7AtTimeIsCappedAtTheLastMillisecondOf9999(t *testing.T) {
	last := time.Date(9999, 12, 31, 23, 59, 59, 999_000_000, time.UTC)
	id, err := NewV7AtTime(last)
	if err != nil {
		t.Fatal(err)
	}
	if got, _ := V7UnixMillis(id); got != 253_402_300_799_999 {
		t.Errorf("embedded %d, want 253402300799999", got)
	}
	for _, tm := range []time.Time{
		time.Date(10000, 1, 1, 0, 0, 0, 0, time.UTC),
		time.Unix(-1, 0),
		// unixSec*1000 == 2^64 + 384 wraps time.Time.UnixMilli to 384: a year-584-million date
		// must not come back as 1970-01-01T00:00:00.384Z.
		time.Unix(18_446_744_073_709_552, 0),
	} {
		if _, err := NewV7AtTime(tm); !errors.Is(err, ErrTimestampOutOfRange) {
			t.Errorf("%v: err = %v, want ErrTimestampOutOfRange", tm, err)
		}
	}
}

func TestGetTimestampHasNoneForAV7DatedPastTheCap(t *testing.T) {
	past := uuid.MustParse("ffffffff-ffff-7000-8000-010203040506")
	if _, err := GetTimestamp(past); !errors.Is(err, ErrNotTimeBased) {
		t.Errorf("GetTimestamp err = %v, want ErrNotTimeBased", err)
	}
	if got, _ := V7UnixMillis(past); got != 0x0000_FFFF_FFFF_FFFF {
		t.Errorf("the field reader read %d, want 281474976710655", got)
	}
	at, err := NewV7At(253_402_300_799_999)
	if err != nil {
		t.Fatal(err)
	}
	if got, err := GetTimestamp(at); err != nil || got.UnixMilli() != 253_402_300_799_999 {
		t.Errorf("GetTimestamp at the cap = %v, %v", got, err)
	}
}
