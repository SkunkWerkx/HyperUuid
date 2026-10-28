//! Helper for `tests/apple_fork.rs`: does a forked child repeat its parent's next draw?
//!
//! The parent advances the generator, forks, and both sides draw four v4 ids. The child sends
//! its 64 bytes up a pipe. Exit codes: 0 the draws differ, 1 they are equal, 2 the 5 s bound
//! expired, 3 any other error. Every non-zero exit prints one line to stderr. Apple only; on
//! any other target it says so and exits 0.

#[cfg(not(target_vendor = "apple"))]
fn main() {
    println!("apple_fork is Apple-only; nothing to do on this target");
}

#[cfg(target_vendor = "apple")]
fn fail(code: i32, msg: &str) -> ! {
    eprintln!("apple_fork: {msg}");
    std::process::exit(code)
}

#[cfg(target_vendor = "apple")]
fn four_ids() -> [u8; 64] {
    let mut out = [0u8; 64];
    for chunk in out.chunks_mut(16) {
        match hyperuuid::v4::new_v4() {
            Ok(id) => chunk.copy_from_slice(id.as_bytes()),
            Err(e) => fail(3, &format!("new_v4 failed: {e}")),
        }
    }
    out
}

#[cfg(target_vendor = "apple")]
fn main() {
    use std::time::{Duration, Instant};

    const BOUND: Duration = Duration::from_secs(5);

    // Advance the generator in the parent first, so state inherited by the child is mid-stream.
    for _ in 0..3 {
        if let Err(e) = hyperuuid::v4::new_v4() {
            fail(3, &format!("new_v4 failed: {e}"));
        }
    }
    let mut buf = [0u8; 16_000];
    for count in [1u32, 37, 1000] {
        let out = &mut buf[..count as usize * 16];
        if let Err(e) = hyperuuid::v7::new_v7_batch(1_700_000_000_000, count, out) {
            fail(3, &format!("new_v7_batch({count}) failed: {e}"));
        }
    }

    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: fds is a live array of two ints.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        fail(3, "pipe failed");
    }
    let (rd, wr) = (fds[0], fds[1]);

    // SAFETY: the process is single-threaded here; the child only draws, writes and exits.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        fail(3, "fork failed");
    }
    if pid == 0 {
        // Child. `_exit` skips the parent's atexit handlers and buffered state.
        let ids = four_ids_child();
        let ok = match ids {
            Some(ids) => {
                // SAFETY: ids is a live 64-byte array.
                unsafe { libc::write(wr, ids.as_ptr().cast(), ids.len()) == ids.len() as isize }
            }
            None => false,
        };
        // SAFETY: terminating the child without unwinding.
        unsafe { libc::_exit(if ok { 0 } else { 3 }) };
    }

    // Parent. Closing its write end lets a child that dies without writing show as EOF at once.
    // SAFETY: wr is the pipe's write end, open and owned here.
    unsafe { libc::close(wr) };
    let start = Instant::now();
    let remaining = || BOUND.saturating_sub(start.elapsed());
    let expire = |why: &str| -> ! {
        // SAFETY: pid is our child; reap it before exiting so nothing is left behind.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
            let mut st = 0;
            libc::waitpid(pid, &mut st, 0);
        }
        fail(2, why)
    };

    let mine = four_ids();

    let mut theirs = [0u8; 64];
    let mut got = 0usize;
    while got < theirs.len() {
        let left = remaining();
        if left.is_zero() {
            expire("timed out waiting for the child's draw");
        }
        let mut p = libc::pollfd {
            fd: rd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: p is a live pollfd.
        let n = unsafe { libc::poll(&mut p, 1, left.as_millis().max(1) as libc::c_int) };
        if n < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            fail(3, "poll failed");
        }
        if n == 0 {
            expire("timed out waiting for the child's draw");
        }
        // SAFETY: the destination is the unfilled tail of a live buffer.
        let r = unsafe { libc::read(rd, theirs[got..].as_mut_ptr().cast(), theirs.len() - got) };
        if r < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            fail(3, "read failed");
        }
        if r == 0 {
            fail(3, "the child closed the pipe early");
        }
        got += r as usize;
    }

    loop {
        let mut st = 0;
        // SAFETY: reaping our own child without blocking.
        let r = unsafe { libc::waitpid(pid, &mut st, libc::WNOHANG) };
        if r == pid {
            break;
        }
        if r < 0 {
            fail(3, "waitpid failed");
        }
        if remaining().is_zero() {
            expire("timed out reaping the child");
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    if mine == theirs {
        fail(1, "the child repeated the parent's draw");
    }
}

/// The child's draw: `None` on error, since the child must not print and exit through std.
#[cfg(target_vendor = "apple")]
fn four_ids_child() -> Option<[u8; 64]> {
    let mut out = [0u8; 64];
    for chunk in out.chunks_mut(16) {
        chunk.copy_from_slice(hyperuuid::v4::new_v4().ok()?.as_bytes());
    }
    Some(out)
}
