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
    pub fn detect() -> Self {
        if std::env::var("BREAKOUT_TRANSPORT").as_deref() == Ok("direct") {
            Self::Direct
        } else {
            Self::Shm
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Shm => "shm",
            Self::Direct => "direct",
        }
    }
}

/// Transmit one frame at the current cursor position, then delete the
/// previous frame's image (ADR-0002 double buffering).
/// `image_id` alternates 1/2; `prev_image_id` is the one to delete after.
pub fn send_frame(
    transport: Transport,
    rgb: &[u8],
    w: u32,
    h: u32,
    image_id: u32,
    prev_image_id: u32,
) -> Result<()> {
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

/// `t=d`: base64 the whole frame, chunk into ≤4096-byte pieces, `m=1` except
/// the last (`m=0`). The cursor must already be parked at the image origin.
fn emit_direct(rgb: &[u8], w: u32, h: u32, image_id: u32) -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();

    let encoded = BASE64.encode(rgb);
    let bytes = encoded.as_bytes();
    let mut offset = 0usize;
    let mut first = true;
    loop {
        let end = (offset + CHUNK_BYTES).min(bytes.len());
        let last = end >= bytes.len();
        let m = if last { 0 } else { 1 };
        if first {
            write!(
                lock,
                "\x1b_Ga=T,f=24,s={w},v={h},t=d,i={image_id},p=1,q=2,C=1,m={m};"
            )?;
        } else {
            write!(lock, "\x1b_Gm={m};")?;
        }
        lock.write_all(&bytes[offset..end])?;
        write!(lock, "\x1b\\")?;
        if last {
            break;
        }
        first = false;
        offset = end;
    }
    lock.flush()?;
    Ok(())
}

/// Delete an image by id, so no frame is left in scrollback (ADR-0002).
fn delete_image(image_id: u32) -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    write!(lock, "\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\")?;
    lock.flush()?;
    Ok(())
}
