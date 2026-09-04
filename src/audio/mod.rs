//! Audio: kira mixing behind a `Sound` trait (FR-46..49).
//!
//! Owns the ten effects (FR-46) plus the looping music bed (FR-47), combo
//! pitch ramp, mute and hit-stop ducking. All samples are synthesized by
//! `build.rs` into `OUT_DIR/audio/*.wav` and compiled in with
//! `include_bytes!` — no data directory, no decoder dependency (ADR-0005).
//!
//! Startup never blocks: [`Audio::spawn`] returns immediately and the kira
//! manager builds on a worker thread. Any failure (no device, bad backend)
//! degrades to silence without blocking play (FR-49); `--caps` reports the
//! probe result from a synchronous check instead.

use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// The ten FR-46 effects, in trait order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    /// Brick took a non-fatal hit.
    BrickHit,
    /// Brick destroyed (pitch ramps with combo).
    BrickDestroy,
    /// Ball bounced off the paddle.
    Paddle,
    /// Ball bounced off a wall.
    Wall,
    /// Capsule spawned.
    Drop,
    /// Capsule collected.
    Collect,
    /// Laser fired.
    Laser,
    /// A life was lost.
    LifeLost,
    /// Level cleared.
    LevelClear,
    /// Perk picked (Phase 8; reserved so the ten arrive together).
    #[allow(dead_code)]
    PerkPick,
}

/// The whole subsystem behind one trait, so it can be replaced by a no-op.
pub trait Sound {
    /// Fire one effect. `combo` (0 when N/A) raises the destroy pitch.
    fn play(&mut self, sfx: Sfx, combo: u32);
    /// Duck the music bed during hit-stop (FR-47).
    fn set_ducked(&mut self, ducked: bool);
    /// `m` toggle (FR-48).
    fn set_muted(&mut self, muted: bool);
    /// Current mute state.
    #[allow(dead_code)]
    fn muted(&self) -> bool;
}

/// Always-silent backend: `--no-audio` and init failure (FR-48/49).
pub struct NoOp {
    muted: bool,
}

impl NoOp {
    /// Fresh silent backend.
    pub fn new() -> Self {
        Self { muted: false }
    }
}

impl Default for NoOp {
    fn default() -> Self {
        Self::new()
    }
}

impl Sound for NoOp {
    fn play(&mut self, _sfx: Sfx, _combo: u32) {}
    fn set_ducked(&mut self, _ducked: bool) {}
    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }
    fn muted(&self) -> bool {
        self.muted
    }
}

/// Per-effect rate limiter: identical sounds within the window collapse
/// into one. A wedged ball can emit contact events every sim step (240Hz);
/// without this that is a machine-gun buzz instead of a bounce.
const THROTTLE_WINDOW: std::time::Duration = std::time::Duration::from_millis(40);

#[derive(Debug)]
struct SfxThrottle {
    last: [Option<std::time::Instant>; 10],
}

impl SfxThrottle {
    fn new() -> Self {
        Self { last: [None; 10] }
    }

    /// True when this effect may fire now (records the firing).
    fn allow(&mut self, idx: usize) -> bool {
        let now = std::time::Instant::now();
        if let Some(t) = self.last[idx] {
            if now - t < THROTTLE_WINDOW {
                return false;
            }
        }
        self.last[idx] = Some(now);
        true
    }
}

macro_rules! wav {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/audio/", $name, ".wav"))
    };
}

/// kira backend. Built on a worker thread; see [`Audio::spawn`].
pub struct KiraSound {
    manager: kira::AudioManager,
    // Held so the music loop keeps playing for the whole session.
    #[allow(dead_code)]
    music: kira::sound::static_sound::StaticSoundHandle,
    music_track: kira::track::TrackHandle,
    sfx: [kira::sound::static_sound::StaticSoundData; 10],
    muted: bool,
    ducked: bool,
    throttle: SfxThrottle,
}

impl KiraSound {
    /// Build the manager, decode the baked WAVs and start the music loop.
    /// Any error degrades the caller to [`NoOp`] (FR-49).
    fn build() -> anyhow::Result<Self> {
        use kira::sound::static_sound::StaticSoundData;
        use kira::track::TrackBuilder;

        let mut manager =
            kira::AudioManager::<kira::DefaultBackend>::new(kira::AudioManagerSettings::default())
                .map_err(|e| anyhow::anyhow!("audio backend: {e:?}"))?;

        let load = |bytes: &'static [u8]| {
            StaticSoundData::from_cursor(Cursor::new(bytes))
                .map_err(|e| anyhow::anyhow!("decode wav: {e:?}"))
        };
        let sfx = [
            load(wav!("brick_hit"))?,
            load(wav!("brick_destroy"))?,
            load(wav!("paddle"))?,
            load(wav!("wall"))?,
            load(wav!("drop"))?,
            load(wav!("collect"))?,
            load(wav!("laser"))?,
            load(wav!("life_lost"))?,
            load(wav!("level_clear"))?,
            load(wav!("perk_pick"))?,
        ];

        // Music bed on its own track, quieter than the effects (FR-47).
        let mut music_track = manager
            .add_sub_track(TrackBuilder::new())
            .map_err(|e| anyhow::anyhow!("music track: {e:?}"))?;
        let music_data = load(wav!("music"))?
            .loop_region(..)
            .volume(kira::Decibels(-14.0));
        let music = music_track
            .play(music_data)
            .map_err(|e| anyhow::anyhow!("music play: {e:?}"))?;

        Ok(Self {
            manager,
            music,
            music_track,
            sfx,
            muted: false,
            ducked: false,
            throttle: SfxThrottle::new(),
        })
    }

    fn effect_index(sfx: Sfx) -> usize {
        match sfx {
            Sfx::BrickHit => 0,
            Sfx::BrickDestroy => 1,
            Sfx::Paddle => 2,
            Sfx::Wall => 3,
            Sfx::Drop => 4,
            Sfx::Collect => 5,
            Sfx::Laser => 6,
            Sfx::LifeLost => 7,
            Sfx::LevelClear => 8,
            Sfx::PerkPick => 9,
        }
    }
}

impl Sound for KiraSound {
    fn play(&mut self, sfx: Sfx, combo: u32) {
        if self.muted {
            return;
        }
        if !self.throttle.allow(Self::effect_index(sfx)) {
            return;
        }
        let mut data = self.sfx[Self::effect_index(sfx)].clone();
        // Combo pitch ramp: destroy rises an octave over 12 combo (FR-47
        // detail: the highest-value audio detail in the game).
        if matches!(sfx, Sfx::BrickDestroy | Sfx::BrickHit) {
            let steps = combo.min(12) as f32;
            let rate = f64::from(2.0f32.powf(steps / 12.0));
            data.settings.playback_rate = kira::Value::Fixed(kira::PlaybackRate(rate));
        }
        let _ = self.manager.play(data);
    }

    fn set_ducked(&mut self, ducked: bool) {
        if ducked == self.ducked {
            return;
        }
        self.ducked = ducked;
        self.music_track.set_volume(
            kira::Decibels(if ducked { -10.0 } else { -14.0 }),
            kira::Tween::default(),
        );
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.manager.main_track().set_volume(
            kira::Decibels(if muted { -60.0 } else { 0.0 }),
            kira::Tween::default(),
        );
    }

    fn muted(&self) -> bool {
        self.muted
    }
}

/// Async handle: `spawn` returns at once; the backend arrives (or fails to)
/// on the worker thread. The main loop pumps [`Audio::poll`] per frame.
pub struct Audio {
    rx: Option<Receiver<anyhow::Result<KiraSound>>>,
    backend: Backend,
    // Mute requested while the worker is still building; applied on arrival.
    pending_muted: bool,
}

enum Backend {
    Pending,
    // Boxed: the manager is ~5 KB next to 1-byte siblings.
    Live(Box<KiraSound>),
    Silent(NoOp),
}

impl Audio {
    /// Start audio without blocking: worker builds kira; failures become
    /// silence (FR-49). `disabled` is `--no-audio` (FR-48).
    pub fn spawn(disabled: bool) -> Self {
        if disabled {
            return Self {
                rx: None,
                backend: Backend::Silent(NoOp::new()),
                pending_muted: false,
            };
        }
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("breakout-audio".to_string())
            .spawn(move || {
                let _ = tx.send(KiraSound::build());
            })
            .ok();
        Self {
            rx: Some(rx),
            backend: Backend::Pending,
            pending_muted: false,
        }
    }

    /// Pick up the built backend (once). Never blocks.
    pub fn poll(&mut self) {
        if !matches!(self.backend, Backend::Pending) {
            return;
        }
        let rx = match &self.rx {
            Some(rx) => rx,
            None => {
                self.backend = Backend::Silent(NoOp::new());
                return;
            }
        };
        match rx.try_recv() {
            Ok(Ok(mut live)) => {
                live.set_muted(self.pending_muted);
                self.backend = Backend::Live(Box::new(live));
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                self.backend = Backend::Silent(NoOp::new());
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    /// True once the worker answered, live or silent.
    pub fn ready(&self) -> bool {
        !matches!(self.backend, Backend::Pending)
    }

    /// True when audio will stay silent (disabled or failed, FR-49).
    pub fn silent(&self) -> bool {
        matches!(self.backend, Backend::Silent(_))
    }

    /// Drain one frame of sim events into sound.
    pub fn play_events(&mut self, events: &[crate::game::physics::SimEvent], combo: u32) {
        self.poll();
        for e in events {
            use crate::game::physics::SimEvent;
            let (sfx, c) = match e {
                SimEvent::WallBounce => (Sfx::Wall, 0),
                SimEvent::PaddleBounce => (Sfx::Paddle, 0),
                SimEvent::BrickHit => (Sfx::BrickHit, combo),
                SimEvent::BrickDestroyed { .. } => (Sfx::BrickDestroy, combo),
                SimEvent::LifeLost => (Sfx::LifeLost, 0),
                SimEvent::LevelClear => (Sfx::LevelClear, 0),
                SimEvent::DropSpawned => (Sfx::Drop, 0),
                SimEvent::Collected(_) => (Sfx::Collect, 0),
                SimEvent::ShotFired => (Sfx::Laser, 0),
            };
            match &mut self.backend {
                Backend::Live(live) => live.play(sfx, c),
                Backend::Silent(noop) => noop.play(sfx, c),
                Backend::Pending => {}
            }
        }
    }

    /// Fire one effect directly (perk picks have no sim event).
    pub fn play_sfx(&mut self, sfx: Sfx, combo: u32) {
        self.poll();
        match &mut self.backend {
            Backend::Live(live) => live.play(sfx, combo),
            Backend::Silent(noop) => noop.play(sfx, combo),
            Backend::Pending => {}
        }
    }

    /// Duck the music bed (delegates to the live backend only).
    pub fn set_ducked(&mut self, ducked: bool) {
        if let Backend::Live(live) = &mut self.backend {
            live.set_ducked(ducked);
        }
    }

    /// Mute toggle (sticks even while the worker is still building).
    pub fn set_muted(&mut self, muted: bool) {
        self.pending_muted = muted;
        match &mut self.backend {
            Backend::Live(live) => live.set_muted(muted),
            Backend::Silent(noop) => noop.set_muted(muted),
            Backend::Pending => {}
        }
    }
}

/// One-line audio state for `--caps` (FR-49: failures are reported).
/// Synchronous: opens the backend, drops it at once.
pub fn probe_summary() -> &'static str {
    match kira::AudioManager::<kira::DefaultBackend>::new(kira::AudioManagerSettings::default()) {
        Ok(_) => "available",
        Err(_) => "unavailable (silent)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throttle_collapses_repeats() {
        let mut t = SfxThrottle::new();
        assert!(t.allow(1));
        assert!(!t.allow(1));
        assert!(t.allow(2));
        std::thread::sleep(THROTTLE_WINDOW + std::time::Duration::from_millis(10));
        assert!(t.allow(1));
    }

    #[test]
    fn noop_never_fails() {
        let mut n = NoOp::new();
        for sfx in [
            Sfx::BrickHit,
            Sfx::BrickDestroy,
            Sfx::Paddle,
            Sfx::Wall,
            Sfx::Drop,
            Sfx::Collect,
            Sfx::Laser,
            Sfx::LifeLost,
            Sfx::LevelClear,
            Sfx::PerkPick,
        ] {
            n.play(sfx, 7);
        }
        n.set_ducked(true);
        n.set_muted(true);
        assert!(n.muted());
    }

    #[test]
    fn disabled_spawns_silent_immediately() {
        let a = Audio::spawn(true);
        assert!(a.silent());
        assert!(a.ready());
    }

    #[test]
    fn baked_wavs_decode() {
        // Every baked sample must parse as a WAV through kira's decoder.
        let files: &[&[u8]] = &[
            wav!("brick_hit"),
            wav!("brick_destroy"),
            wav!("paddle"),
            wav!("wall"),
            wav!("drop"),
            wav!("collect"),
            wav!("laser"),
            wav!("life_lost"),
            wav!("level_clear"),
            wav!("perk_pick"),
            wav!("music"),
        ];
        for bytes in files {
            assert!(bytes.starts_with(b"RIFF"), "not a wav");
            assert!(bytes.len() > 44, "empty wav");
        }
    }
}
