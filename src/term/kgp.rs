//! Kitty graphics escape writer and transports (ADR-0002).
//!
//! Owns the two frame transports:
//! - `t=s`: reference a POSIX shared-memory segment created by `shm.rs`.
//! - `t=d`: base64 payload in 4096-byte chunks (`m=1` except the last `m=0`),
//!   used automatically when shm fails and forced by `BREAKOUT_TRANSPORT=direct`.
//!
//! Also owns image-id double buffering: transmit the new frame (id A), then
//! delete the previous frame (id B) — never delete before transmit, or the
//! screen flashes (ADR-0002).

use std::io::Write;

use anyhow::Result;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use super::shm;

/// Maximum direct-payload chunk, in *decoded* bytes (ADR-0002: 4096).
const CHUNK_BYTES: usize = 4096;

/// Transport selection for the session. `direct` forced by the test-only
/// env switch `BREAKOUT_TRANSPORT=direct`; otherwise shm is the primary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Shm,
    Direct,
}

impl Transport {
    /// Env-forced mode for testing (`BREAKOUT_TRANSPORT=direct`).
    pub fn detect() -> Self {
        if std::env::var("BREAKOUT_TRANSPORT").as_deref() == Ok("direct") {
            Self::Direct
        } else {
            Self::Shm
        }
    }

    /// Session selection: env-forced direct wins; otherwise a 1x1 shm
    /// probe decides. Ghostty's default image limits reject `t=s`, so
    /// probing once up front (instead of failing every frame silently
    /// under `q=2`) is what makes the game visible there.
    pub fn select() -> Self {
        if Self::detect() == Self::Direct {
            return Self::Direct;
        }
        if super::caps::probe_shm() {
            Self::Shm
        } else {
            Self::Direct
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Shm => "shm",
            Self::Direct => "direct",
        }
    }
}

/// Transmit one frame, then delete the previous frame's image (ADR-0002
/// double buffering). `image_id` alternates 1/2; `prev_image_id` is the
/// one to delete after.
///
/// The image anchors at the cursor, so the cursor is homed first every
/// frame — without this the frame lands wherever the shell left the
/// cursor (usually the bottom) and the screen stays empty.
pub fn send_frame(
    transport: Transport,
    rgb: &[u8],
    w: u32,
    h: u32,
    image_id: u32,
    prev_image_id: u32,
) -> Result<()> {
    home_cursor()?;
    match transport {
        Transport::Shm => {
            // If shm fails for any reason, fall back to direct for this and
            // future frames without dying (ADR-0002: correctness over speed).
            match shm::transmit(rgb, w, h, image_id as usize) {
                Ok(seg) => emit_shm(&seg.name_b64, w, h, image_id)?,
                Err(_) => emit_direct(rgb, w, h, image_id)?,
            }
        }
        Transport::Direct => emit_direct(rgb, w, h, image_id)?,
    }
    delete_image(prev_image_id)?;
    Ok(())
}

/// Park the cursor at the top-left cell so the frame fills the window.
fn home_cursor() -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    write!(lock, "\x1b[H")?;
    lock.flush()?;
    Ok(())
}

/// `t=s`: `ESC _G a=T,f=24,s=<w>,v=<h>,t=s,i=<id>,p=1,q=2,C=1;<base64 name> ESC \`
fn emit_shm(name_b64: &str, w: u32, h: u32, image_id: u32) -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    write!(
        lock,
        "\x1b_Ga=T,f=24,s={w},v={h},t=s,i={image_id},p=1,q=2,C=1;{name_b64}\x1b\\"
    )?;
    lock.flush()?;
    Ok(())
}

// Reused base64 scratch: a scale-4 frame encodes to ~5 MB, and
// allocating that every frame at 60 fps is pure allocator churn.
thread_local! {
    static B64_BUF: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Chunk plan for a base64 payload: `(offset, len, last)` pieces of at
/// most [`CHUNK_BYTES`] bytes. Pure (tested): lengths are multiples of 4
/// so no quantum ever splits, and reassembly is byte-exact.
fn chunk_plan(total: usize) -> Vec<(usize, usize, bool)> {
    let mut plan = Vec::new();
    let mut offset = 0usize;
    loop {
        let end = (offset + CHUNK_BYTES).min(total);
        let last = end >= total;
        plan.push((offset, end - offset, last));
        if last {
            break;
        }
        offset = end;
    }
    plan
}

/// `t=d`: base64 the whole frame into the reused scratch, then chunk into
/// ≤4096-byte pieces, `m=1` except the last (`m=0`). The cursor must
/// already be parked at the image origin.
fn emit_direct(rgb: &[u8], w: u32, h: u32, image_id: u32) -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    B64_BUF.with(|cell| {
        let mut buf = cell.borrow_mut();
        buf.clear();
        BASE64.encode_string(rgb, &mut buf);
        let mut first = true;
        for (offset, len, last) in chunk_plan(buf.len()) {
            let m = if last { 0 } else { 1 };
            if first {
                write!(
                    lock,
                    "\x1b_Ga=T,f=24,s={w},v={h},t=d,i={image_id},p=1,q=2,C=1,m={m};"
                )?;
                first = false;
            } else {
                write!(lock, "\x1b_Gm={m};")?;
            }
            lock.write_all(&buf.as_bytes()[offset..offset + len])?;
            write!(lock, "\x1b\\")?;
        }
        lock.flush()?;
        Ok(())
    })
}

/// Delete an image by id, so no frame is left in scrollback (ADR-0002).
fn delete_image(image_id: u32) -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    write!(lock, "\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\")?;
    lock.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_reassemble_exactly() {
        for total in [0usize, 4, 4092, 4096, 4100, 8192, 5_000_000] {
            let plan = chunk_plan(total);
            let mut at = 0usize;
            for (i, (off, len, last)) in plan.iter().enumerate() {
                assert_eq!(*off, at);
                assert!(*len <= CHUNK_BYTES);
                assert_eq!(*len % 4, 0, "split quantum at piece {i}");
                assert_eq!(*last, i + 1 == plan.len());
                at += len;
            }
            assert_eq!(at, total);
        }
    }

    #[test]
    fn scratch_encode_matches_fresh() {
        // Buffer reuse must not change a single byte on the wire.
        let rgb = vec![0xABu8; 1000];
        let fresh = BASE64.encode(&rgb);
        B64_BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            buf.clear();
            BASE64.encode_string(&rgb, &mut buf);
            assert_eq!(buf.as_str(), fresh.as_str());
            // Second fill reuses without leftovers.
            let small = vec![1u8; 10];
            buf.clear();
            BASE64.encode_string(&small, &mut buf);
            assert_eq!(buf.as_str(), BASE64.encode(&small).as_str());
        });
    }
}
