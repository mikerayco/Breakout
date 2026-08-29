//! Turns game state into pixels (Phase 1+).
//!
//! Owns the framebuffer, tiny-skia drawing, the 5x7 bitmap font, particles,
//! camera, bloom, palette and the HUD. Owns the frame orchestration: pick
//! scale from window size (ADR-0003), rasterise, hand bytes to the transport,
//! keep the double-buffer image id, and drive the presentation clock.
//!
//! Must not import `game/` simulation modules except read-only state
//! (ADR-0005: rendering never mutates the simulation).

pub mod bloom;
pub mod camera;
pub mod draw;
pub mod framebuffer;
pub mod hud;
pub mod palette;
pub mod particles;
pub mod text;

use std::collections::VecDeque;
use std::time::Instant;

use crate::term::kgp::Transport;

pub use framebuffer::{compute_scale, Framebuffer};

/// The presentation clock and the double-buffer image-id bookkeeping.
///
/// Drops, never queues (FR-9): if the loop falls behind, the next frame is
/// due immediately rather than piling up.
pub struct Frames {
    pub fps: u32,
    next_due: Instant,
    last_presented: Instant,
    pub image_id: u32,
    pub transport: Transport,
    /// Rolling frame times (ms) for the p50/p99 overlay.
    times: VecDeque<f32>,
}

impl Frames {
    pub fn new(fps: u32, transport: Transport) -> Self {
        Self {
            fps: fps.clamp(30, 144),
            next_due: Instant::now(),
            last_presented: Instant::now(),
            image_id: 1,
            transport,
            times: VecDeque::with_capacity(128),
        }
    }

    /// The absolute time the next frame is due.
    pub fn next_due(&self) -> Instant {
        self.next_due
    }

    /// How long to wait before the next frame is due.
    pub fn wait_until_next(&self) -> std::time::Duration {
        self.next_due.saturating_duration_since(Instant::now())
    }

    /// Record that a frame is being presented now. If we're already past
    /// due (dropping), the next_due clock snaps to `now + interval`: frames
    /// are dropped, never queued (FR-9). Returns the frame time in ms.
    pub fn record_presented(&mut self, now: Instant) -> f32 {
        let dt_ms = now.duration_since(self.last_presented).as_secs_f32() * 1000.0;
        self.last_presented = now;
        if self.times.len() >= 128 {
            self.times.pop_front();
        }
        self.times.push_back(dt_ms);
        let interval = std::time::Duration::from_secs_f64(1.0 / self.fps as f64);
        self.next_due = now + interval;
        dt_ms
    }

    /// Alternate the image id for the next frame (1 → 2 → 1 …).
    /// Returns (new_id, previous_id_to_delete).
    pub fn next_image_id(&mut self) -> (u32, u32) {
        self.image_id = if self.image_id == 1 { 2 } else { 1 };
        let prev = if self.image_id == 1 { 2 } else { 1 };
        (self.image_id, prev)
    }

    /// p50/p99 of recorded frame times (ms), for the overlay.
    pub fn percentiles(&self) -> (f32, f32) {
        if self.times.is_empty() {
            return (0.0, 0.0);
        }
        let mut sorted: Vec<f32> = self.times.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p50 = sorted[sorted.len() / 2];
        let idx = ((sorted.len() - 1) as f32 * 0.99).round() as usize;
        let p99 = sorted[idx];
        (p50, p99)
    }

    /// Average fps over the recorded window.
    pub fn avg_fps(&self) -> f32 {
        if self.times.is_empty() {
            return 0.0;
        }
        1000.0 / (self.times.iter().sum::<f32>() / self.times.len() as f32)
    }
}
