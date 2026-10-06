//! Entropy for every generator: [`fill`] is `getrandom::fill`, except on Linux, where it is
//! ChaCha20 run in user space in one of two shapes, and on Android, which has the first.
//!
//! **Large requests (a batch of 32 v7 or 24 v6 UUIDs and up), Linux and Android:** one 32-byte
//! `getrandom` draw keys a ChaCha20 keystream that fills the whole request. That is the
//! construction the kernel itself uses behind `getrandom` (a ChaCha20 key per request, fresh
//! from the entropy pool), run here with AVX2/SSE2/NEON instead of the kernel's
//! one-block-at-a-time code. The key is used once and the nonce is fixed at zero, which is what
//! a single-use key allows, and nothing outlives the call. The key is not wiped afterwards: all
//! it can reproduce is this batch's keystream, and every bit of that keystream is either handed
//! to the caller in the UUIDs or overwritten by their version and variant bits.
//!
//! **Small requests (every single-item generator, and smaller batches), Linux:** a pool of
//! buffered ChaCha20 generators, [`pool`] below, which has the details. A single UUID needs 6
//! to 16 bytes, too few to pay for keying a cipher per call, so the pool keys one per kilobyte
//! of keystream and hands it out a few bytes at a time. No key outlives a second of wall-clock
//! time, which is what keeps a process restored from a snapshot from replaying its bytes.
//!
//! Measured on linux-x64 (i9-11900H): `new_v7_batch` of 1000 went from 10.6 to 4.4 ns per UUID
//! on glibc 2.43, whose `getrandom` already runs in the vDSO, and from 12.5 to 3.4 on musl,
//! whose `getrandom` is a real syscall; a single `new_v4` went from 38 to 23 ns on glibc and
//! from 228 to 24 on musl. The other targets keep `getrandom` because it was already as fast or
//! faster there: Windows' `ProcessPrng` did the same batch in 2.4 ns per UUID against
//! ChaCha20's 3.4, and wasm32-wasip1 under wasmtime came out even. macOS has not been measured
//! yet.
//!
//! Keying a cipher for a large request costs ~700 cycles (the AVX2 backend works eight blocks
//! at a time even for one), which the pool beats below about 192 bytes.

/// Fills `buf` with cryptographically secure random bytes.
pub(crate) fn fill(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    fill_with(buf, None)
}

/// [`fill`] for a caller that already holds a time, as v6 and v7 do: `unix_millis` is the
/// timestamp going into the UUIDs being made. On Linux a timestamp within a second of the
/// slot's key spares the pool its own clock read; any other sends it to the clock, which alone
/// retires a key (see [`pool`]).
pub(crate) fn fill_at(buf: &mut [u8], unix_millis: u64) -> Result<(), getrandom::Error> {
    fill_with(buf, Some(unix_millis))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[inline(always)]
fn fill_with(buf: &mut [u8], unix_millis: Option<u64>) -> Result<(), getrandom::Error> {
    use chacha20::ChaCha20;
    use chacha20::cipher::{KeyIvInit, StreamCipher};

    /// The request size at which keying a cipher per request beats the pool.
    const STRETCH_THRESHOLD: usize = 192;

    if buf.len() < STRETCH_THRESHOLD {
        #[cfg(target_os = "linux")]
        return pool::fill(buf, unix_millis);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = unix_millis;
            return getrandom::fill(buf);
        }
    }

    let mut key = [0u8; 32];
    getrandom::fill(&mut key)?;
    // The keystream is XORed in, so the buffer is zeroed first to leave the keystream itself.
    // `try_` because `apply_keystream` panics past the end of the keystream (256 GiB, far more
    // than a batch can ask for): the optimizer only proves that while this is inlined into a
    // batch function whose length it can see, and the no-panic proof should not depend on it.
    buf.fill(0);
    ChaCha20::new(&key.into(), &[0u8; 12].into())
        .try_apply_keystream(buf)
        .map_err(|_| getrandom::Error::UNEXPECTED)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
#[inline(always)]
fn fill_with(buf: &mut [u8], _unix_millis: Option<u64>) -> Result<(), getrandom::Error> {
    getrandom::fill(buf)
}

/// The small-request generator: a fixed pool of ChaCha20 generators, each a kilobyte of
/// buffered keystream, in one anonymous mapping created on first use.
///
/// Each slot follows the fast-key-erasure design of the kernel's own vDSO `getrandom` and of
/// OpenBSD's `arc4random`. A refill runs ChaCha20 under the slot's key over 1,024 bytes; the
/// first 32 replace the key, which is never used twice, and the other 992 are handed out in
/// order, each byte zeroed as it leaves. So nothing in memory can recover a value already
/// handed out. The chain starts from a `getrandom` draw and is cut off, and started again from
/// a new one, after 1,024 refills (about a megabyte per slot) or one second of wall-clock
/// time, whichever comes first.
///
/// **Fork.** A forked child gets a copy of the parent's memory, and with it every slot's
/// unread keystream; both processes would then hand out the same bytes. The mapping is marked
/// `MADV_WIPEONFORK`, the mechanism the vDSO `getrandom` relies on for the same reason, so the
/// child sees it zero-filled: every slot reads as unseeded and draws a fresh key from the OS on
/// first use. A kernel without it (before 4.14) refuses the advice, and the pool is not used.
///
/// **Snapshots.** A process restored from a snapshot (a cloned virtual machine, AWS Lambda
/// SnapStart, CRaC and CRIU checkpoints) resumes with the memory it had, in as many copies as
/// are restored, and nothing tells it so; the kernel's own generator is told, through the VM
/// generation ID, and this one is not. What it has instead is the one-second limit above. A
/// slot is stamped with the wall clock when it draws a key from the OS, and every draw checks
/// the stamp first: a key more than a second old, or stamped in the future, is thrown away with
/// everything buffered under it, and the bytes come from a new `getrandom` key. So a snapshot
/// taken more than a second after a slot's last key hands every copy a slot that is already
/// stale by its own clock, whether or not anything corrects that clock on the way back up.
/// What is left is a snapshot taken within a second of a key being drawn and restored onto a
/// clock that has not been moved on: the copies then share bytes until that second runs out.
///
/// The clock is `CLOCK_REALTIME_COARSE`, a few nanoseconds through the vDSO. v6 and v7 arrive
/// holding a timestamp, and when that timestamp is within the second the slot is taken to be
/// fresh without the clock being read. A timestamp can only save the read: one that disagrees
/// (a backfill minting UUIDs for old rows, a fixed test value) sends the question to the clock,
/// which alone decides that a key is stale, so such a caller pays for the read and nothing
/// else. The one thing a timestamp can get wrong is to vouch for a key the clock would have
/// retired, which takes a timestamp within a second of the key's stamp that is not the current
/// time: after a restore, a constant captured before the snapshot.
///
/// **Threads.** A `no_std` library has no thread-locals, so a thread is pointed at a slot by a
/// hash of its thread pointer, which is unique among live threads, and takes the slot with a
/// try-lock. A busy slot sends it to the next, and after four busy slots, or if the pool could
/// not be created, the request goes to `getrandom` directly. Nothing ever waits, so a signal
/// handler that generates a UUID while its own thread holds a slot gets another slot or the OS,
/// never a deadlock.
#[cfg(target_os = "linux")]
mod pool {
    use chacha20::ChaCha20;
    use chacha20::cipher::{KeyIvInit, StreamCipher};
    use core::cell::UnsafeCell;
    use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

    /// Keystream bytes per refill, the first 32 of which become the next key.
    const STREAM: usize = 1024;
    /// Slots in the pool: 64 of 1,088 bytes, 68 KiB of address space, resident only as touched.
    const SLOTS: usize = 64;
    /// Busy slots tried before falling back to `getrandom`.
    const PROBES: usize = 4;
    /// Refills between fresh `getrandom` keys.
    const RESEED_EVERY: u32 = 1024;
    /// Milliseconds of wall-clock time a key may be used for, in either direction from its
    /// stamp: a clock set back by more than this retires the key as well.
    const STALE_MILLIS: u64 = 1000;

    const _: () = assert!(SLOTS.is_power_of_two());

    /// A slot's generator. All-zero bytes are a valid value meaning "unseeded", which is what a
    /// fresh mapping and a forked child's wiped one both hold.
    #[repr(C)]
    #[cfg_attr(test, derive(Clone))]
    struct Generator {
        /// The wall clock, in Unix-epoch milliseconds, when `key`'s chain was drawn from the OS.
        keyed_at: u64,
        seeded: u32,
        pos: u32,
        refills: u32,
        key: [u8; 32],
        stream: [u8; STREAM],
    }

    /// One cache-line-aligned slot, so two threads on neighbouring slots never share a line.
    #[repr(C, align(64))]
    struct Slot {
        lock: AtomicU32,
        generator: UnsafeCell<Generator>,
    }

    const POOL_BYTES: usize = SLOTS * core::mem::size_of::<Slot>();

    /// Null until first use; then the mapping, or [`UNAVAILABLE`] if one could not be made.
    static POOL: AtomicPtr<Slot> = AtomicPtr::new(core::ptr::null_mut());
    /// A sentinel no mapping can have (`mmap` never returns the first page).
    const UNAVAILABLE: *mut Slot = core::ptr::dangling_mut::<Slot>();

    fn pool() -> *mut Slot {
        let current = POOL.load(Ordering::Acquire);
        if !current.is_null() {
            return current;
        }
        // SAFETY: a fresh anonymous private mapping with no address hint; checked before use.
        let mapped = unsafe {
            libc::mmap(
                core::ptr::null_mut(),
                POOL_BYTES,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        let mine = if mapped == libc::MAP_FAILED {
            UNAVAILABLE
        // Advice that does not exist has to be refused before advice that does can be believed:
        // qemu's user-mode emulation used to answer every `madvise` with success and act on
        // none, which would leave a forked child holding its parent's keystream. BoringSSL
        // makes the same check for the same reason.
        // SAFETY: advice on the mapping just made, of exactly its length.
        } else if unsafe { libc::madvise(mapped, POOL_BYTES, -1) } == 0
            || unsafe { libc::madvise(mapped, POOL_BYTES, libc::MADV_WIPEONFORK) } != 0
        {
            // Without wipe-on-fork a child would replay the parent's keystream; see above.
            // SAFETY: unmaps the mapping just made, which nothing else has seen.
            unsafe { libc::munmap(mapped, POOL_BYTES) };
            UNAVAILABLE
        } else {
            mapped.cast::<Slot>()
        };
        match POOL.compare_exchange(
            core::ptr::null_mut(),
            mine,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => mine,
            Err(winner) => {
                // Another thread got there first; its mapping is the pool.
                if mine != UNAVAILABLE {
                    // SAFETY: as above, ours and unseen.
                    unsafe { libc::munmap(mine.cast(), POOL_BYTES) };
                }
                winner
            }
        }
    }

    pub(super) fn fill(buf: &mut [u8], unix_millis: Option<u64>) -> Result<(), getrandom::Error> {
        let base = pool();
        if base == UNAVAILABLE {
            return getrandom::fill(buf);
        }
        let hash = thread_pointer().wrapping_mul(0x9E37_79B9_7F4A_7C15_u64 as usize);
        let start = hash >> (usize::BITS - SLOTS.trailing_zeros());
        for probe in 0..PROBES {
            let index = start.wrapping_add(probe) % SLOTS;
            // SAFETY: `index < SLOTS`, so this is inside the mapping, which is never unmapped
            // once published and whose zero-filled bytes are a valid `Slot`.
            let slot = unsafe { &*base.add(index) };
            if slot
                .lock
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // SAFETY: holding the lock gives this thread the generator exclusively.
                let generator = unsafe { &mut *slot.generator.get() };
                let result = take(generator, buf, unix_millis, coarse_millis);
                slot.lock.store(0, Ordering::Release);
                return result;
            }
        }
        getrandom::fill(buf)
    }

    /// The calling thread's thread pointer, unique among live threads, or on an architecture
    /// without one wired up here, its stack address, which spreads threads almost as well.
    #[inline(always)]
    fn thread_pointer() -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            let tp: usize;
            // SAFETY: `%fs:0` is the thread control block's pointer to itself, which glibc and
            // musl both set up for every thread before any code of ours can run on it.
            unsafe {
                core::arch::asm!(
                    "mov {}, fs:0",
                    out(reg) tp,
                    options(nostack, readonly, preserves_flags)
                )
            };
            tp
        }
        #[cfg(target_arch = "aarch64")]
        {
            let tp: usize;
            // SAFETY: reads the thread pointer register, which has no side effects.
            unsafe {
                core::arch::asm!(
                    "mrs {}, tpidr_el0",
                    out(reg) tp,
                    options(nomem, nostack, preserves_flags)
                )
            };
            tp
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            let marker = 0u8;
            (core::ptr::addr_of!(marker) as usize) >> 16
        }
    }

    /// The wall clock in Unix-epoch milliseconds, to the kernel's last tick (1 to 10 ms): all a
    /// one-second limit needs, at a quarter of what the exact clock costs.
    fn coarse_millis() -> Option<u64> {
        // SAFETY: `timespec` is plain integers, for which all zeroes is a valid value, and
        // `clock_gettime` writes only through the pointer it is given.
        let mut ts: libc::timespec = unsafe { core::mem::zeroed() };
        if unsafe { libc::clock_gettime(libc::CLOCK_REALTIME_COARSE, &mut ts) } != 0 {
            return None;
        }
        Some(
            (ts.tv_sec as u64)
                .wrapping_mul(1000)
                .wrapping_add(ts.tv_nsec as u64 / 1_000_000),
        )
    }

    /// Whether a key stamped `keyed_at` may still be used at `now`.
    #[inline(always)]
    fn fresh(keyed_at: u64, now: u64) -> bool {
        now.abs_diff(keyed_at) <= STALE_MILLIS
    }

    /// Starts the slot's chain again from a `getrandom` key, stamped `now`. Everything
    /// buffered under the old key is abandoned unread, and overwritten by the next refill.
    fn rekey(generator: &mut Generator, now: u64) -> Result<(), getrandom::Error> {
        // Unseeded while the key is being replaced, so a draw that fails part-way leaves a slot
        // the next call keys again, never one with half a key.
        generator.seeded = 0;
        getrandom::fill(&mut generator.key)?;
        generator.keyed_at = now;
        generator.refills = 0;
        generator.pos = STREAM as u32;
        generator.seeded = 1;
        Ok(())
    }

    fn refill(generator: &mut Generator) -> Result<(), getrandom::Error> {
        generator.stream.fill(0);
        ChaCha20::new(&generator.key.into(), &[0u8; 12].into())
            .try_apply_keystream(&mut generator.stream)
            .map_err(|_| getrandom::Error::UNEXPECTED)?;
        let (next_key, _) = generator.stream.split_at_mut(32);
        generator.key.copy_from_slice(next_key);
        next_key.fill(0);
        generator.pos = 32;
        generator.refills += 1;
        Ok(())
    }

    /// Hands `out` its bytes from the slot. `unix_millis` is the caller's timestamp, if it has
    /// one, and `clock` reads the wall clock; it is a parameter so the tests can move time.
    #[inline(always)]
    fn take(
        generator: &mut Generator,
        mut out: &mut [u8],
        unix_millis: Option<u64>,
        clock: impl FnOnce() -> Option<u64>,
    ) -> Result<(), getrandom::Error> {
        let due = generator.seeded == 0 || generator.refills >= RESEED_EVERY;
        // The caller's timestamp can vouch for the key and save the clock read. It cannot
        // retire one: only the clock does that (the module docs have why).
        if due || !unix_millis.is_some_and(|at| fresh(generator.keyed_at, at)) {
            // A clock that cannot be read cannot vouch for anything buffered.
            let Some(now) = clock() else {
                return getrandom::fill(out);
            };
            if due || !fresh(generator.keyed_at, now) {
                rekey(generator, now)?;
            }
        }
        while !out.is_empty() {
            if generator.pos as usize >= STREAM {
                refill(generator)?;
            }
            let unread = generator
                .stream
                .get_mut(generator.pos as usize..)
                .unwrap_or_default();
            let n = unread.len().min(out.len());
            let (src, _) = unread.split_at_mut(n);
            let (dst, rest) = core::mem::take(&mut out).split_at_mut(n);
            // One loop rather than `copy_from_slice` and `fill`: those become calls to memcpy
            // and memset, which on musl cost more than the rest of a six-byte draw.
            for (to, from) in dst.iter_mut().zip(src.iter_mut()) {
                *to = *from;
                *from = 0;
            }
            generator.pos += n as u32;
            out = rest;
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Some time in 2026; the tests move the clock around it.
        const T: u64 = 1_790_000_000_000;

        /// A slot keyed at [`T`] with keystream buffered, as a snapshot would catch it.
        fn keyed_slot() -> Generator {
            // SAFETY: all-zero bytes are a valid, unseeded `Generator`.
            let mut slot: Generator = unsafe { core::mem::zeroed() };
            draw(&mut slot, None, Some(T));
            assert_eq!((slot.seeded, slot.keyed_at), (1, T));
            slot
        }

        fn draw(slot: &mut Generator, unix_millis: Option<u64>, clock: Option<u64>) -> [u8; 16] {
            let mut out = [0u8; 16];
            take(slot, &mut out, unix_millis, || clock).unwrap();
            out
        }

        /// One draw from each of two copies of `slot`, which is what two restores of one
        /// snapshot are, with nothing but the clock or the timestamp to tell them apart.
        fn two_restores(
            slot: &Generator,
            unix_millis: Option<u64>,
            clock: Option<u64>,
        ) -> ([u8; 16], [u8; 16]) {
            let (mut a, mut b) = (slot.clone(), slot.clone());
            (
                draw(&mut a, unix_millis, clock),
                draw(&mut b, unix_millis, clock),
            )
        }

        #[test]
        fn a_key_is_retired_after_a_second_in_either_direction() {
            let slot = keyed_slot();
            // The control: inside the second two copies replay each other, so the checks
            // below do see a restore, and it is the stamp that separates the copies.
            let (a, b) = two_restores(&slot, None, Some(T + STALE_MILLIS));
            assert_eq!(
                a, b,
                "copies of a fresh slot should hand out the same bytes"
            );
            let (a, b) = two_restores(&slot, None, Some(T - STALE_MILLIS));
            assert_eq!(
                a, b,
                "copies of a fresh slot should hand out the same bytes"
            );

            let (a, b) = two_restores(&slot, None, Some(T + STALE_MILLIS + 1));
            assert_ne!(a, b, "a key over a second old was used");
            let (a, b) = two_restores(&slot, None, Some(T - STALE_MILLIS - 1));
            assert_ne!(
                a, b,
                "a key stamped over a second ahead of the clock was used"
            );
        }

        #[test]
        fn a_rekeyed_slot_is_stamped_and_fresh_again() {
            let mut slot = keyed_slot();
            draw(&mut slot, None, Some(T + 5_000));
            assert_eq!((slot.seeded, slot.keyed_at), (1, T + 5_000));
            let (a, b) = two_restores(&slot, None, Some(T + 5_000 + STALE_MILLIS));
            assert_eq!(a, b);
        }

        #[test]
        fn a_current_timestamp_saves_the_clock_read() {
            let slot = keyed_slot();
            let mut copy = slot.clone();
            let mut out = [0u8; 16];
            take(&mut copy, &mut out, Some(T + STALE_MILLIS), || {
                panic!("the clock was read")
            })
            .unwrap();
            assert_eq!(out, draw(&mut slot.clone(), None, Some(T)));
        }

        #[test]
        fn an_old_timestamp_asks_the_clock_and_only_the_clock_retires_the_key() {
            let slot = keyed_slot();
            // A backfill: the timestamp is years off, the clock says the key is fresh. The
            // key stays (rekeying on every such call would cost more than the pool saves).
            let old = Some(T - 100_000_000_000);
            let (a, b) = two_restores(&slot, old, Some(T));
            assert_eq!(a, b, "an old timestamp alone retired the key");
            // The same timestamp once the clock has moved past the second.
            let (a, b) = two_restores(&slot, old, Some(T + STALE_MILLIS + 1));
            assert_ne!(a, b, "a key over a second old was used");
        }

        #[test]
        fn a_megabyte_of_keystream_retires_the_key_inside_the_second() {
            // Two copies drawn in step under a clock that never moves replay each other
            // until the refill count, and nothing else, rekeys them.
            let slot = keyed_slot();
            let (mut a, mut b) = (slot.clone(), slot.clone());
            let mut bytes = 0usize;
            while draw(&mut a, None, Some(T)) == draw(&mut b, None, Some(T)) {
                bytes += 16;
                assert!(bytes <= 2 * RESEED_EVERY as usize * STREAM, "never rekeyed");
            }
            let refills = bytes / (STREAM - 32);
            assert!(
                (RESEED_EVERY as usize - 2..=RESEED_EVERY as usize).contains(&refills),
                "rekeyed after {refills} refills"
            );
        }

        #[test]
        fn an_unreadable_clock_sends_the_draw_to_the_os() {
            let slot = keyed_slot();
            let (a, b) = two_restores(&slot, None, None);
            assert_ne!(
                a, b,
                "buffered keystream was used with no clock to vouch for it"
            );
            // And a timestamp that cannot vouch (it is stale) does not stand in for the clock.
            let (a, b) = two_restores(&slot, Some(T + STALE_MILLIS + 1), None);
            assert_ne!(a, b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fill;

    // Both sides of the stretch threshold, and a length that ends mid-block.
    const LENGTHS: [usize; 4] = [16, 191, 192, 6_003];

    #[test]
    fn two_draws_never_repeat() {
        // A fixed key, or a key that was never mixed in, would repeat across calls even though
        // every item inside one batch still differs (which is all the batch tests check).
        for len in LENGTHS {
            let (mut a, mut b) = (vec![0u8; len], vec![0u8; len]);
            fill(&mut a).unwrap();
            fill(&mut b).unwrap();
            assert_ne!(a, b, "two {len}-byte draws matched");
        }
    }

    #[test]
    fn every_byte_is_written_and_the_bits_are_balanced() {
        // Starting from all-ones catches a draw that leaves the buffer as it was, or overwrites
        // it with a constant. The count of one bits in n random bits has a standard
        // deviation of sqrt(n)/2; six of them is a one-in-500-million flake, and still well short
        // of the n/2 a stuck buffer is off by, even at 16 bytes.
        for len in LENGTHS {
            let mut buf = vec![0xFFu8; len];
            fill(&mut buf).unwrap();
            let ones: u32 = buf.iter().map(|b| b.count_ones()).sum();
            let bits = (len * 8) as f64;
            let half = bits / 2.0;
            let slack = 6.0 * bits.sqrt() / 2.0;
            assert!(
                (f64::from(ones) - half).abs() < slack,
                "{len} bytes: {ones} one bits, expected about {half}"
            );
        }
    }

    #[test]
    fn small_draws_spanning_refills_never_repeat() {
        // Thousands of 16-byte draws walk one slot through many refills, so a key that failed
        // to roll forward would replay a whole kilobyte and show up here as a repeat.
        let draws: std::collections::HashSet<[u8; 16]> = (0..5_000)
            .map(|_| {
                let mut b = [0u8; 16];
                fill(&mut b).unwrap();
                b
            })
            .collect();
        assert_eq!(draws.len(), 5_000);
    }
}
