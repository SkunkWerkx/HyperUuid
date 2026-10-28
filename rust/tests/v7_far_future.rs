//! A process whose v7 stream was pushed far into the future by one call still mints from the
//! clock. Real-time calls then fall more than the clamp window behind the stream, so they are
//! minted out of stream: each one must still succeed and be a valid version 7 whose timestamp is
//! within a second of the clock.
//!
//! Its own integration test binary because the stream is process-global: in the unit test
//! process this call would make every later in-stream test mint out of stream.

// `now_v7` has no clock to read on wasm32.
#![cfg(not(target_arch = "wasm32"))]

use std::time::{SystemTime, UNIX_EPOCH};

use hyperuuid::v7;

// The stream's last millisecond, 2^44 − 2.
const STREAM_MAX_MILLIS: u64 = (1 << 44) - 2;

fn wall_clock_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[test]
fn real_time_calls_after_a_future_dated_call_still_succeed() {
    v7::new_v7(STREAM_MAX_MILLIS).unwrap();
    for _ in 0..1_000 {
        let id = v7::now_v7().unwrap();
        let now = wall_clock_millis();
        assert!(id.is_rfc(7), "{id} is not an RFC 9562 version 7");
        let stamped = v7::unix_millis(&id);
        assert!(
            stamped.abs_diff(now) <= 1_000,
            "stamped {stamped}, the clock reads {now}"
        );
    }
}
