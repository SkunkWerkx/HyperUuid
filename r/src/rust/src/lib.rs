// no_std: the core's `staticlib` feature supplies the panic handler, so the archive R links
// carries no copy of Rust's standard library. `extern crate` keeps the core's #[no_mangle]
// C exports in this staticlib even though nothing here calls them.
#![no_std]

extern crate hyperuuid;
