//! The link-time proof behind the `no-panic` feature. `#[no_panic]` only reports a function
//! when a release-optimized binary that calls it is linked, and a cdylib does not count: it
//! drops every function no export reaches. So this calls each annotated native once, with
//! inputs the optimizer cannot see through, and building it is the whole check. It needs
//! unwind, because under abort a panic path is invisible to `no-panic`:
//!
//!     CARGO_PROFILE_RELEASE_PANIC=unwind cargo build --release --locked --example no_panic --features no-panic
//!
//! A panic path left in any of them fails the link with `ERROR[no-panic]` naming the
//! function. Running the binary proves nothing: the calls sit behind a condition that is
//! never true.

use core::hint::black_box;
use core::ptr::null_mut;

fn main() {
    if std::env::args().len() > 1000 {
        let env = black_box(null_mut());
        let class = black_box(null_mut());
        let msb = black_box(0_i64);
        let lsb = black_box(0_i64);
        let int = black_box(0_i32);
        let arr = black_box(null_mut());
        unsafe {
            black_box(hyperuuid_jni::native_uuid_new_v4(env, class, arr));
            black_box(hyperuuid_jni::native_uuid_new_v5(
                env, class, msb, lsb, arr, arr,
            ));
            black_box(hyperuuid_jni::native_uuid_new_v7(env, class, msb, arr));
            black_box(hyperuuid_jni::native_uuid_v7_unix_millis(
                env, class, msb, lsb,
            ));
            black_box(hyperuuid_jni::native_uuid_v7_unix_millis_in(
                env, class, msb, lsb, int,
            ));
            black_box(hyperuuid_jni::native_uuid_new_v7_batch(
                env, class, msb, int, arr,
            ));
            black_box(hyperuuid_jni::native_uuid_v7_to_sql_order(
                env, class, msb, lsb, arr,
            ));
            black_box(hyperuuid_jni::native_uuid_v7_to_rfc_order(
                env, class, msb, lsb, arr,
            ));
            black_box(hyperuuid_jni::native_uuid_version(
                env, class, msb, lsb, int,
            ));
            black_box(hyperuuid_jni::native_uuid_variant(env, class, msb, lsb));
            black_box(hyperuuid_jni::native_uuid_is_rfc(
                env, class, msb, lsb, int, int,
            ));
            black_box(hyperuuid_jni::native_uuid_get_timestamp(
                env, class, msb, lsb, int, arr,
            ));
            black_box(hyperuuid_jni::native_hyperuuid_version(env, class));
            black_box(hyperuuid_jni::native_jni_natives(env, class));
            black_box(hyperuuid_jni::JNI_OnLoad(
                black_box(null_mut()),
                black_box(null_mut()),
            ));
        }
    }
}
