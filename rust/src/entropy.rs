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
//! of keystream and hands it out a few bytes at a time.
//!
//! Measured on linux-x64 (i9-11900H): `new_v7_batch` of 1000 went from 8.9 to 3.5 ns per UUID
//! on glibc 2.43, whose `getrandom` already runs in the vDSO, and from 12.5 to 3.4 on musl,
//! whose `getrandom` is a real syscall; a single `new_v4` went from 38 to 18.5 ns on glibc and
//! from 228 to 18 on musl. The other targets keep `getrandom` because it was already as fast or
//! faster there: Windows' `ProcessPrng` did the same batch in 2.4 ns per UUID against
//! ChaCha20's 3.4, and wasm32-wasip1 under wasmtime came out even. macOS has not been measured
//! yet.
//!
//! Keying a cipher for a large request costs ~700 cycles (the AVX2 backend works eight blocks
//! at a time even for one), which the pool beats below about 192 bytes.

/// Fills `buf` with cryptographically secure random bytes.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn fill(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    use chacha20::ChaCha20;
    use chacha20::cipher::{KeyIvInit, StreamCipher};

    /// The request size at which keying a cipher per request beats the pool.
    const STRETCH_THRESHOLD: usize = 192;

    if buf.len() < STRETCH_THRESHOLD {
        #[cfg(target_os = "linux")]
        return pool::fill(buf);
        #[cfg(not(target_os = "linux"))]
        return getrandom::fill(buf);
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

/// Fills `buf` with cryptographically secure random bytes.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub(crate) fn fill(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    getrandom::fill(buf)
}

/// The small-request generator: a fixed pool of ChaCha20 generators, each a kilobyte of
/// buffered keystream, in one anonymous mapping created on first use.
///
/// Each slot follows the fast-key-erasure design of the kernel's own vDSO `getrandom` and of
/// OpenBSD's `arc4random`. A refill runs ChaCha20 under the slot's key over 1,024 bytes; the
/// first 32 replace the key, which is never used twice, and the other 992 are handed out in
/// order, each byte zeroed as it leaves. So nothing in memory can recover a value already
/// handed out, and every 1,024 refills (about a megabyte per slot) the key is replaced with
/// a fresh `getrandom` draw instead.
///
/// **Fork.** A forked child gets a copy of the parent's memory, and with it every slot's
/// unread keystream; both processes would then hand out the same bytes. The mapping is marked
/// `MADV_WIPEONFORK`, the mechanism the vDSO `getrandom` relies on for the same reason, so the
/// child sees it zero-filled: every slot reads as unseeded and draws a fresh key from the OS on
/// first use. A kernel without it (before 4.14) refuses the advice, and the pool is not used.
///
/// **What this does not cover.** A virtual machine cloned from a snapshot resumes with the
/// same memory, and nothing tells the pool, so both copies hand out the same bytes until each
/// slot's next fresh key. The kernel's own generator detects that through the vDSO; this does
/// not. That trade was made knowingly for the speed above.
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

    const _: () = assert!(SLOTS.is_power_of_two());

    /// A slot's generator. All-zero bytes are a valid value meaning "unseeded", which is what a
    /// fresh mapping and a forked child's wiped one both hold.
    #[repr(C)]
    struct Generator {
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
        // SAFETY: advice on the mapping just made, of exactly its length.
        } else if unsafe { libc::madvise(mapped, POOL_BYTES, libc::MADV_WIPEONFORK) } != 0 {
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

    pub(super) fn fill(buf: &mut [u8]) -> Result<(), getrandom::Error> {
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
                let result = take(unsafe { &mut *slot.generator.get() }, buf);
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

    fn refill(generator: &mut Generator) -> Result<(), getrandom::Error> {
        if generator.seeded == 0 || generator.refills >= RESEED_EVERY {
            getrandom::fill(&mut generator.key)?;
            generator.refills = 0;
            generator.seeded = 1;
        }
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

    fn take(generator: &mut Generator, mut out: &mut [u8]) -> Result<(), getrandom::Error> {
        if generator.seeded == 0 {
            generator.pos = STREAM as u32;
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
