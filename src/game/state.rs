//! Game states: Title / Playing / Paused / LevelClear / RunOver (FR-6, FR-21).
//!
//! Owns the state machine and its legal transitions. A pure enum + move
//! logic; no I/O.

/// Top-level game state (PRD FR-6, FR-20, FR-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    /// Title screen; `Space`/confirm starts, `q` quits (FR-6).
    Title,
    /// Ball(s) in play or waiting launch.
    Playing,
    /// `Esc` during play; Resume / Restart / Mute / Quit (FR-6).
    Paused,
    /// No destructible bricks remain (FR-21); advances to next level/perk.
    LevelClear,
    /// Lives reached 0 (FR-20) or run finished; shows summary.
    RunOver,
}

impl GameState {
    /// Legal transitions. Returns the new state, or `None` if illegal.
    pub fn transition(self, event: StateEvent) -> Option<Self> {
        use GameState::{LevelClear, Paused, Playing, RunOver, Title};
        match (self, event) {
            (Title, StateEvent::Start) => Some(Playing),
            (Title, StateEvent::Quit) => Some(RunOver),
            (Playing, StateEvent::Pause) => Some(Paused),
            (Playing, StateEvent::ClearLevel) => Some(LevelClear),
            (Playing, StateEvent::LoseRun) => Some(RunOver),
            (Paused, StateEvent::Resume) => Some(Playing),
            (Paused, StateEvent::Quit) => Some(RunOver),
            (Paused, StateEvent::Restart) => Some(Playing),
            (LevelClear, StateEvent::Start) => Some(Playing),
            (LevelClear, StateEvent::Quit) => Some(RunOver),
            (RunOver, StateEvent::Start) => Some(Playing),
            (RunOver, StateEvent::Quit) => Some(RunOver),
            _ => None,
        }
    }
}

/// Events that drive the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateEvent {
    /// Confirm on title / advance from LevelClear / restart from RunOver.
    Start,
    /// `Esc` in play.
    Pause,
    /// `Esc` in pause, or resume item.
    Resume,
    /// Restart item in pause.
    Restart,
    /// Last destructible brick destroyed.
    ClearLevel,
    /// Lives reached 0.
    LoseRun,
    /// Quit to shell.
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_starts_or_quits() {
        assert_eq!(
            GameState::Title.transition(StateEvent::Start),
            Some(GameState::Playing)
        );
        assert_eq!(
            GameState::Title.transition(StateEvent::Quit),
            Some(GameState::RunOver)
        );
        assert_eq!(GameState::Title.transition(StateEvent::Pause), None);
    }

    #[test]
    fn pause_resume_cycle() {
        assert_eq!(
            GameState::Playing.transition(StateEvent::Pause),
            Some(GameState::Paused)
        );
        assert_eq!(
            GameState::Paused.transition(StateEvent::Resume),
            Some(GameState::Playing)
        );
    }
}
