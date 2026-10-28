//! The v7 stream used up at its last millisecond, through the public API: batches at that
//! millisecond stay in order until the stream cannot advance, and then the call fails with
//! `CounterExhausted` instead of minting out of order.
//!
//! Its own integration test binary because the stream is process-global: once it sits at its
//! last millisecond, every later v7 call in the process at a real time mints out of stream.

use hyperuuid::{Uuid, v7};

// The stream's last millisecond, 2^44 − 2.
const STREAM_MAX_MILLIS: u64 = (1 << 44) - 2;
// 1,024 steps of the stream's 2^20 per millisecond, so at most 1,024 batches fit.
const COUNT: u32 = 65_536;

#[test]
fn v7_counter_exhaustion_through_the_public_api() {
    v7::new_v7(STREAM_MAX_MILLIS).unwrap();
    let mut out = vec![0u8; COUNT as usize * 16];
    let item =
        |out: &[u8], i: usize| Uuid::from_bytes(out[i * 16..(i + 1) * 16].try_into().unwrap());
    let mut previous_last = None;
    let mut error = None;
    for _ in 0..1_025 {
        match v7::new_v7_batch(STREAM_MAX_MILLIS, COUNT, &mut out) {
            Ok(()) => {
                let (first, last) = (item(&out, 0), item(&out, COUNT as usize - 1));
                if let Some(previous) = previous_last {
                    assert!(
                        first > previous,
                        "{first} <= the previous batch's {previous}"
                    );
                }
                previous_last = Some(last);
            }
            Err(e) => {
                error = Some(e);
                break;
            }
        }
    }
    let error = error.expect("1,025 batches at the last millisecond all succeeded");
    assert_eq!(error, v7::NewV7Error::CounterExhausted);
    assert_eq!(
        error.to_string(),
        "version 7 counter exhausted: the stream cannot advance without running more than 1000 ms ahead of the supplied timestamp or past its last millisecond"
    );
    assert!(previous_last.is_some(), "the first batch already failed");
}
