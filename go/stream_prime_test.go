package hyperuuid

import "time"

// The v7 stream is process-wide and starts at zero, so the first call at the fixed 2022 RFC
// vector would join it, and later calls at the vector would be clamped or split ahead of it.
// One call at the present moves the stream there, which leaves the vector more than 11 s
// behind it: minted out of stream, carrying its timestamp exactly. Tests that check ordering
// mint at the present, within a second of it, and never drag the stream ahead.
//
// The rule: no test supplies more than now + 500 ms, and current-time bounds allow + 1000 ms.
// A call further ahead drags the stream into the future. The fixed-vector tests don't notice,
// since the vector stays more than 11 s behind any stream at or past the present; the
// current-time tests do: within 11 s they clamp onto a later millisecond and fail a tight
// bound, and beyond it they are minted out of stream and lose their ordering.
func init() {
	if _, err := NewV7At(uint64(time.Now().UnixMilli())); err != nil {
		panic("priming the v7 stream failed: " + err.Error())
	}
}
