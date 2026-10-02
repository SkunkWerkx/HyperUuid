//! The link-time proof behind the `no-panic` feature. `#[no_panic]` only reports a function
//! when a release-optimized binary that calls it is linked, and a cdylib does not count: it
//! drops every function no export reaches. So this calls each annotated function once, with
//! inputs the optimizer cannot see through, and building it is the whole check:
//!
//!     cargo build --release --example no_panic --features no-panic
//!
//! A panic path left in any of them fails the link with `ERROR[no-panic]` naming the
//! function. Running the binary proves nothing further.

use core::hint::black_box;
use core::str::FromStr;
use hyperuuid::{Timestamp, Uuid, get_timestamp, hyperuuid_version, v4, v5, v6, v7};

// The C exports, reached by symbol the way every binding reaches them.
unsafe extern "C" {
    fn uuid_new_v4(out_ptr: *mut u8) -> i32;
    fn uuid_new_v5(ns_ptr: *const u8, name_ptr: *const u8, name_len: u32, out_ptr: *mut u8) -> i32;
    fn uuid_new_v6(unix_millis: u64, out_ptr: *mut u8) -> i32;
    fn uuid_v6_unix_millis(uuid_ptr: *const u8) -> u64;
    fn uuid_new_v6_batch(unix_millis: u64, count: u32, out_ptr: *mut u8) -> i32;
    fn uuid_v6_to_sql_order(uuid_ptr: *mut u8);
    fn uuid_v6_to_rfc_order(uuid_ptr: *mut u8);
    fn uuid_new_v7(unix_millis: u64, out_ptr: *mut u8) -> i32;
    fn uuid_v7_unix_millis(uuid_ptr: *const u8) -> u64;
    fn uuid_new_v7_batch(unix_millis: u64, count: u32, out_ptr: *mut u8) -> i32;
    fn uuid_v7_to_sql_order(uuid_ptr: *mut u8);
    fn uuid_v7_to_rfc_order(uuid_ptr: *mut u8);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count = black_box(args.len() as u32);
    let millis = black_box(args.len() as u64);
    let text = black_box(args[0].as_str());
    let mut out = vec![0u8; args.len() * 16];

    let id = Uuid::from_str(text).unwrap_or(Uuid::NIL);
    let at = Timestamp::from_unix_millis(millis);
    black_box(v4::new_v4().is_ok());
    black_box(v5::new_v5(id, text.as_bytes()));
    black_box(v6::new_v6(millis).is_ok());
    black_box(v6::new_v6_at(at).is_ok());
    black_box(v6::new_v6_batch(millis, count, &mut out).is_ok());
    black_box(v6::unix_millis(&id));
    black_box(v6::to_sql_order(&id));
    black_box(v6::to_rfc_order(&id));
    black_box(v7::now_v7().is_ok());
    black_box(v7::new_v7(millis).is_ok());
    black_box(v7::new_v7_at(at).is_ok());
    black_box(v7::new_v7_batch(millis, count, &mut out).is_ok());
    black_box(v7::unix_millis(&id));
    black_box(v7::to_sql_order(&id));
    black_box(v7::to_rfc_order(&id));
    black_box(get_timestamp(&id));
    black_box(hyperuuid_version());

    let ptr = out.as_mut_ptr();
    // SAFETY: `out` holds `count` UUIDs, at least one, since args[0] always exists.
    unsafe {
        black_box(uuid_new_v4(ptr));
        black_box(uuid_new_v5(ptr, text.as_ptr(), text.len() as u32, ptr));
        black_box(uuid_new_v6(millis, ptr));
        black_box(uuid_v6_unix_millis(ptr));
        black_box(uuid_new_v6_batch(millis, count, ptr));
        uuid_v6_to_sql_order(ptr);
        uuid_v6_to_rfc_order(ptr);
        black_box(uuid_new_v7(millis, ptr));
        black_box(uuid_v7_unix_millis(ptr));
        black_box(uuid_new_v7_batch(millis, count, ptr));
        uuid_v7_to_sql_order(ptr);
        uuid_v7_to_rfc_order(ptr);
    }
}
