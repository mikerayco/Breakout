//! POSIX shared memory frame transport (ADR-0002, Phase 1).
//!
//! Owns `shm_open`/`ftruncate`/`mmap`/`munmap` and the `t=s` transmit
//! escape. One of only two modules allowed `unsafe` (PRD NFR-8); every
//! `unsafe` block carries a comment naming its invariant.
#![allow(unsafe_code)]

pub fn stub() -> ! {
    todo!("term/shm: implemented in Phase 1")
}
