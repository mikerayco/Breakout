//! POSIX shared-memory frame transport (ADR-0002).
//!
//! Owns `shm_open`/`ftruncate`/`mmap`/`munmap`/`close` for the `t=s`
//! transmit path, rotating across three object names so the terminal's
//! read-then-unlink never races a slow reader. One of only two modules
//! allowed `unsafe` (PRD NFR-8); every `unsafe` block names its invariant.
//!
//! The escape-sequence emission lives in `kgp.rs`; this module only moves
//! the RGB bytes into a shared segment and returns the base64 name.
#![allow(unsafe_code)]

use std::ffi::CString;
use std::os::unix::io::RawFd;

use anyhow::{Context, Result};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

/// Number of rotating shm names (ADR-0002: three).
const SLOTS: usize = 3;

/// An open, filled, closed shared segment ready to reference by name.
pub struct ShmSegment {
    pub name_b64: String,
}

fn object_name(pid: i64, slot: usize) -> Result<CString> {
    CString::new(format!("/bkout-{pid}-{slot}"))
        .context("shm name contains NUL (impossible with numeric args)")
}

/// Create the shm object, write `rgb` into it, and close it.
/// The terminal reads it by name and unlinks it (ADR-0002).
pub fn transmit(rgb: &[u8], w: u32, h: u32, slot: usize) -> Result<ShmSegment> {
    let len = (w as usize) * (h as usize) * 3;
    anyhow::ensure!(rgb.len() == len, "shm size mismatch: {}/{}", rgb.len(), len);

    let name = object_name(std::process::id() as i64, slot % SLOTS)?;
    let fd = create_segment(&name)?;
    if let Err(e) = map_write_close(fd, rgb) {
        unsafe { libc::close(fd) };
        return Err(e);
    }
    let name_b64 = BASE64.encode(name.to_bytes());
    Ok(ShmSegment { name_b64 })
}

/// Best-effort unlink of a segment by its base64 name (startup probing).
/// Failures are ignored: on success the terminal already unlinked it.
pub fn unlink_b64(name_b64: &str) {
    let Ok(raw) = BASE64.decode(name_b64) else {
        return;
    };
    let Ok(name) = CString::new(raw) else {
        return;
    };
    // SAFETY: shm_unlink on a valid NUL-terminated name; return ignored.
    unsafe {
        libc::shm_unlink(name.as_ptr());
    }
}

/// SAFETY INVARIANT: `name` is a CString with no interior NUL; `shm_open` is
/// called exactly as documented; the returned fd is valid until closed.
fn create_segment(name: &CString) -> Result<RawFd> {
    // SAFETY: shm_open with O_CREAT|O_RDWR|0600 on a valid name; the returned
    // fd is owned by this module and closed on every path.
    let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o600) };
    if fd < 0 {
        anyhow::bail!(
            "shm_open {} failed: {}",
            name.to_string_lossy(),
            std::io::Error::last_os_error()
        );
    }
    Ok(fd)
}

/// SAFETY INVARIANT: `fd` is an open shm fd; `rgb.len()` equals the object
/// size. Writes the bytes into the mmap and closes the fd.
fn map_write_close(fd: RawFd, rgb: &[u8]) -> Result<()> {
    // SAFETY: ftruncate on a valid shm fd; checked return.
    if unsafe { libc::ftruncate(fd, rgb.len() as libc::off_t) } != 0 {
        anyhow::bail!("ftruncate failed: {}", std::io::Error::last_os_error());
    }
    // SAFETY: mmap PROT_READ|PROT_WRITE MAP_SHARED on a freshly truncated
    // object; the returned map is valid for rgb.len() bytes while live.
    let addr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            rgb.len(),
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };
    if addr == libc::MAP_FAILED {
        anyhow::bail!("mmap failed: {}", std::io::Error::last_os_error());
    }
    // SAFETY: addr points to rgb.len() writable bytes; rgb has that many;
    // the copy is byte-exact into the shared region.
    unsafe {
        std::ptr::copy_nonoverlapping(rgb.as_ptr(), addr as *mut u8, rgb.len());
    }
    // SAFETY: munmap of the exact region we mapped.
    unsafe {
        libc::munmap(addr, rgb.len());
    }
    // SAFETY: close the fd now the map is gone.
    unsafe {
        libc::close(fd);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_name_is_stable_and_unique_per_slot() {
        let a = object_name(123, 0).unwrap();
        let b = object_name(123, 1).unwrap();
        assert_ne!(a, b);
        assert!(a.to_bytes().starts_with(b"/bkout-123-"));
    }

    #[test]
    fn transmit_roundtrips_through_segment_name() {
        let rgb = vec![7u8; 3 * 4 * 4]; // 4x4 RGB
        let seg = transmit(&rgb, 4, 4, 0).unwrap();
        assert!(!seg.name_b64.is_empty());
        let decoded = BASE64.decode(&seg.name_b64).unwrap();
        let s = String::from_utf8(decoded).unwrap();
        assert!(s.starts_with("/bkout-"));
    }
}
