//! One RAII guard owns all terminal state, and it always runs (ADR-0010).
//!
//! `TerminalGuard` is the only code that enables raw mode, enters the
//! alternate screen, hides the cursor, or pushes keyboard enhancement flags.
//! Nothing else in the crate may take over terminal state.
//!
//! Teardown itself lives in `caps::teardown_terminal_if_needed`, which is
//! idempotent and infallible and is shared with the panic hook (ADR-0010):
//! raw escape writes, no allocation, no formatting — safe to call from a
//! panic.

use crossterm::cursor::Hide;
use crossterm::event::{KeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;

use super::caps;

pub struct TerminalGuard {
    _private: (),
}

impl TerminalGuard {
    /// Take over the terminal: raw mode, alternate screen, hidden cursor,
    /// keyboard enhancement flags — in that order (ADR-0010).
    ///
    /// On any failure partway through, teardown runs before the error is
    /// returned, so a half-taken-over terminal is still restored.
    pub fn enter() -> anyhow::Result<Self> {
        caps::prepare_for_takeover()?;
        // Mark active *before* any state changes so a partial failure still
        // tears down (ADR-0010: no half-taken-over terminal).
        caps::mark_takeover_active();

        let mut stdout = std::io::stdout();
        let setup = (|| -> std::io::Result<()> {
            crossterm::terminal::enable_raw_mode()?;
            execute!(
                stdout,
                EnterAlternateScreen,
                Hide,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                )
            )?;
            Ok(())
        })();

        if let Err(err) = setup {
            caps::teardown_terminal_if_needed();
            anyhow::bail!("terminal takeover failed: {err}");
        }

        Ok(Self { _private: () })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Same idempotent teardown the panic hook uses (ADR-0010).
        caps::teardown_terminal_if_needed();
    }
}
