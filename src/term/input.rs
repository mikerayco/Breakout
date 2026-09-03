//! Keyboard protocol input and the held-key set (ADR-0004).
//!
//! Owns crossterm key events (Press/Repeat/Release), the held-key set, and
//! the repeat-decay fallback when the protocol is unavailable. Game logic
//! reads `Held` + edge actions only; physics constants are untouched
//! (Phase 3 changes feel via input state, not tuning).

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

/// How long a press sustains movement in fallback mode (~140ms, ADR-0004).
pub const LEGACY_HOLD: Duration = Duration::from_millis(140);

/// Forced fallback for testing (`BREAKOUT_INPUT=legacy`).
pub fn legacy_forced() -> bool {
    std::env::var("BREAKOUT_INPUT").as_deref() == Ok("legacy")
}

/// True when the terminal answered the `CSI ? u` query at all. Support is
/// the fact it answered, not any particular flag value (PERF.md Phase 0).
pub fn keyboard_supported() -> bool {
    super::caps::probe_keyboard_flags().is_some()
}

/// Resolve the input mode once at startup: forced legacy, or fallback when
/// the protocol is absent.
pub fn legacy_mode() -> bool {
    legacy_forced() || !keyboard_supported()
}

/// Edge actions collected during one frame. Movement itself is held state,
/// read separately via [`Poller::held`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `Space`: launch a stuck ball / advance a menu.
    Launch,
    /// `Esc`: pause / resume.
    Pause,
    /// `q`: quit to shell.
    Quit,
    /// `m`: mute toggle.
    Mute,
    /// `F3`: debug overlay toggle.
    Debug,
    /// `F4`: bloom toggle (FR-29).
    Bloom,
    /// `F5`: debug spawner — cycle and spawn a chosen powerup (Phase 6).
    Spawn,
    /// Number keys 1-3: perk offer direct pick (Phase 8).
    Pick(usize),
    /// Pause menu navigation.
    MenuUp,
    /// Pause menu navigation.
    MenuDown,
    /// Pause menu confirm (`Enter`/`Space`).
    MenuConfirm,
}

/// Held movement state sampled per frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Held {
    /// Move left held.
    pub left: bool,
    /// Move right held.
    pub right: bool,
}

/// The frame poller: owns the held-key set and per-frame edge actions.
///
/// Protocol mode drives held state from Press/Repeat (insert) and Release
/// (remove). Legacy mode drives it from press deadlines so autorepeat
/// sustains movement (ADR-0004). On unexpected gaps the set can be cleared
/// so the paddle never sticks.
pub struct Poller {
    legacy: bool,
    left_held: bool,
    right_held: bool,
    left_until: Instant,
    right_until: Instant,
    edges: Vec<Action>,
    /// Last movement-key press, for the NFR-2 latency readout.
    pub last_press: Option<Instant>,
}

impl Poller {
    /// New poller in the resolved mode.
    pub fn new(legacy: bool) -> Self {
        let now = Instant::now();
        Self {
            legacy,
            left_held: false,
            right_held: false,
            left_until: now,
            right_until: now,
            edges: Vec::new(),
            last_press: None,
        }
    }

    /// Whether this poller runs the degraded fallback.
    pub fn is_legacy(&self) -> bool {
        self.legacy
    }

    /// Observe one crossterm key event. Movement keys update held state;
    /// everything else becomes an edge [`Action`].
    pub fn observe(&mut self, key: &KeyEvent, now: Instant) {
        let press = matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat);
        let release = matches!(key.kind, KeyEventKind::Release);
        match key.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                if self.legacy {
                    if press {
                        self.left_until = now + LEGACY_HOLD;
                        self.last_press = Some(now);
                    }
                } else if press {
                    self.left_held = true;
                    self.last_press = Some(now);
                } else if release {
                    self.left_held = false;
                }
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                if self.legacy {
                    if press {
                        self.right_until = now + LEGACY_HOLD;
                        self.last_press = Some(now);
                    }
                } else if press {
                    self.right_held = true;
                    self.last_press = Some(now);
                } else if release {
                    self.right_held = false;
                }
            }
            KeyCode::Char(' ') => {
                if press {
                    self.edges.push(Action::Launch);
                }
            }
            KeyCode::Esc => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::Pause);
                }
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::Quit);
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::Mute);
                }
            }
            KeyCode::F(3) => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::Debug);
                }
            }
            KeyCode::F(4) => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::Bloom);
                }
            }
            KeyCode::F(5) => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::Spawn);
                }
            }
            KeyCode::Char('1' | '2' | '3') => {
                if matches!(key.kind, KeyEventKind::Press) {
                    if let KeyCode::Char(c) = key.code {
                        self.edges.push(Action::Pick(c as usize - '1' as usize));
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::MenuUp);
                }
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::MenuDown);
                }
            }
            KeyCode::Enter => {
                if matches!(key.kind, KeyEventKind::Press) {
                    self.edges.push(Action::MenuConfirm);
                }
            }
            _ => {}
        }
    }

    /// Current held movement state.
    pub fn held(&self, now: Instant) -> Held {
        if self.legacy {
            Held {
                left: now < self.left_until,
                right: now < self.right_until,
            }
        } else {
            Held {
                left: self.left_held,
                right: self.right_held,
            }
        }
    }

    /// Drain the edge actions collected since the last frame.
    pub fn take_edges(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.edges)
    }

    /// Clear held state (focus loss / event gap: the paddle must not stick).
    pub fn clear_held(&mut self) {
        self.left_held = false;
        self.right_held = false;
        let now = Instant::now();
        self.left_until = now;
        self.right_until = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    fn release(code: KeyCode) -> KeyEvent {
        KeyEvent::new_with_kind(
            code,
            crossterm::event::KeyModifiers::NONE,
            KeyEventKind::Release,
        )
    }

    #[test]
    fn protocol_hold_and_release() {
        let mut p = Poller::new(false);
        let now = Instant::now();
        p.observe(&press(KeyCode::Left), now);
        assert!(p.held(now).left);
        p.observe(&release(KeyCode::Left), now);
        assert!(!p.held(now).left);
    }

    #[test]
    fn legacy_decay_holds_then_releases() {
        let mut p = Poller::new(true);
        let t0 = Instant::now();
        p.observe(&press(KeyCode::Right), t0);
        assert!(p.held(t0).right);
        assert!(p.held(t0 + LEGACY_HOLD - Duration::from_millis(10)).right);
        assert!(!p.held(t0 + LEGACY_HOLD + Duration::from_millis(10)).right);
    }

    #[test]
    fn edges_collected_for_bindings() {
        let mut p = Poller::new(false);
        let now = Instant::now();
        p.observe(&press(KeyCode::Char(' ')), now);
        p.observe(&press(KeyCode::Esc), now);
        p.observe(&press(KeyCode::Char('m')), now);
        p.observe(&press(KeyCode::F(3)), now);
        p.observe(&press(KeyCode::F(4)), now);
        p.observe(&press(KeyCode::F(5)), now);
        p.observe(&press(KeyCode::Char('2')), now);
        let edges = p.take_edges();
        assert!(edges.contains(&Action::Launch));
        assert!(edges.contains(&Action::Pause));
        assert!(edges.contains(&Action::Mute));
        assert!(edges.contains(&Action::Debug));
        assert!(edges.contains(&Action::Bloom));
        assert!(edges.contains(&Action::Spawn));
        assert!(edges.contains(&Action::Pick(1)));
        assert!(p.take_edges().is_empty());
    }

    #[test]
    fn clear_held_unsticks_paddle() {
        let mut p = Poller::new(false);
        let now = Instant::now();
        p.observe(&press(KeyCode::Left), now);
        p.observe(&press(KeyCode::Right), now);
        p.clear_held();
        assert_eq!(p.held(now), Held::default());
    }
}
