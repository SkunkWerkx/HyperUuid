//! The Linux user-space entropy pool (src/entropy.rs) keeps keystream in memory between
//! calls, which is safe only if no two holders of that memory ever hand out the same bytes.
//! These are the two ways they could: a forked child inheriting its parent's buffered
//! keystream, and two threads sharing one slot.

#![cfg(target_os = "linux")]

use hyperuuid::v4;

#[test]
fn a_forked_child_draws_different_bytes_from_its_parent() {
    // Warm the pool so the parent holds unread keystream at the moment it forks: without
    // `MADV_WIPEONFORK` the child's next draw would be byte-for-byte the parent's.
    for _ in 0..3 {
        v4::new_v4().unwrap();
    }
    let mut fds = [0i32; 2];
    // SAFETY: plain libc calls on a pipe this test owns; the child only writes 16 bytes and
    // exits without unwinding or running the test harness again.
    unsafe {
        assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
        let pid = libc::fork();
        assert!(pid >= 0, "fork failed");
        if pid == 0 {
            let child = v4::new_v4().unwrap().into_bytes();
            libc::write(fds[1], child.as_ptr().cast(), 16);
            libc::_exit(0);
        }
        let parent = v4::new_v4().unwrap().into_bytes();
        let mut child = [0u8; 16];
        assert_eq!(libc::read(fds[0], child.as_mut_ptr().cast(), 16), 16);
        libc::waitpid(pid, core::ptr::null_mut(), 0);
        assert_ne!(parent, child, "the child replayed the parent's keystream");
    }
}

#[test]
fn concurrent_threads_never_draw_the_same_value() {
    // More threads than a slot's probe sequence, all drawing at once, so slots are contended
    // and taken over: a lock that let two threads into one slot would hand both the same bytes.
    let all: Vec<[u8; 16]> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    (0..20_000)
                        .map(|_| v4::new_v4().unwrap().into_bytes())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    });
    let distinct: std::collections::HashSet<_> = all.iter().collect();
    assert_eq!(distinct.len(), all.len());
}
