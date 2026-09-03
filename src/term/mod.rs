//! Everything that talks to the terminal.
//!
//! `caps` probes the terminal and owns the platform plumbing (unsafe allowed,
//! NFR-8). `guard` is the RAII owner of all terminal state (ADR-0010). `shm`
//! and `kgp` transport frames (Phase 1). `input` reads keys (Phase 3).
//!
//! Must not be imported by `game/` (ADR-0005: the simulation is pure).

pub mod caps;
pub mod guard;
pub mod input;
#[allow(dead_code)]
pub mod kgp;
#[allow(dead_code)]
pub mod shm;
