//! Fork safety on Apple targets: a child made by `fork` must never repeat the bytes its parent
//! draws next, even when the parent drew before forking.
//!
//! The check runs in a separate process (`examples/apple_fork.rs`), supervised from here, so a
//! hang or a crash takes the helper down and not the test runner. The helper is in its own
//! process group: on the 15 s bound the whole group is killed, because killing the helper alone
//! would leave a forked child alive, still holding the inherited stderr pipe.
//!
//! `cargo test --test apple_fork` does not build the example. Run `cargo test`, or
//! `cargo build --example apple_fork && cargo test --test apple_fork`.
#![cfg(target_vendor = "apple")]

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn a_forked_child_never_repeats_its_parent() {
    // current_exe is target/<profile>/deps/apple_fork-<hash>; the example is in ../examples.
    let exe = std::env::current_exe().unwrap();
    let helper = exe
        .parent()
        .and_then(|deps| deps.parent())
        .unwrap()
        .join("examples")
        .join("apple_fork");
    assert!(
        helper.exists(),
        "{} is missing: run `cargo build --example apple_fork` (or an unfiltered `cargo test`)",
        helper.display()
    );

    let mut child = Command::new(&helper)
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let pgid = child.id() as libc::pid_t;
    let mut stderr = child.stderr.take().unwrap();
    // Drain stderr on a thread so a chatty helper cannot block on a full pipe.
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(15) {
            // SAFETY: plain syscall on the helper's own process group.
            unsafe { libc::killpg(pgid, libc::SIGKILL) };
            let _ = child.wait();
            let text = reader.join().unwrap();
            panic!("helper did not finish in 15 s; stderr: {text}");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let text = reader.join().unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "helper failed ({status}); stderr: {text}"
    );
}
