//! Breakout — a GPU-accelerated terminal arcade game (PRD §1, ADR-0001).
//!
//! Startup path: parse flags (FR-3), probe capabilities, take over the
//! terminal via `TerminalGuard` (ADR-0010), then run. Every exit path —
//! normal, panic, SIGINT, SIGTERM — restores the terminal.

#![deny(unsafe_code)] // NFR-8: unsafe lives only in term/shm.rs and term/caps.rs

mod cli;
mod term;

// Skeleton modules from ADR-0005 §Layout, compiling with stub bodies until
// their phases (render: 1+, game: 2+, audio: 7, save: 8).
#[allow(dead_code)]
mod audio;
#[allow(dead_code)]
mod game;
#[allow(dead_code)]
mod render;
#[allow(dead_code)]
mod save;

use std::time::Duration;

use clap::Parser;

use crate::cli::Cli;

const FR4_GRAPHICS_MISSING: &str = "\
This terminal does not support the Kitty graphics protocol, which breakout requires.

Known-working terminals: Ghostty, Kitty, WezTerm.";

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // FR-3: --caps prints the capability report and exits 0, before any
    // terminal takeover.
    if cli.caps {
        print!("{}", term::caps::capability_report());
        return Ok(());
    }

    // FR-4: graphics support is mandatory, no text fallback. Probe before
    // taking over the terminal so the message is visible on a clean screen.
    if !term::caps::probe_graphics() {
        eprintln!("{FR4_GRAPHICS_MISSING}");
        std::process::exit(2);
    }

    install_panic_hook();
    let guard = term::guard::TerminalGuard::enter()?;

    // Phase 0 placeholder: a text frame until Phase 1 renders pixels.
    run_placeholder_loop()?;

    drop(guard);
    Ok(())
}

/// Installs a hook that runs the terminal teardown *before* the default
/// panic printer, so a backtrace lands on a usable terminal (ADR-0010).
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        term::caps::teardown_terminal_if_needed();
        default(info);
    }));
}

/// Phase 0 placeholder loop: show a text frame, quit on `q` or a signal.
/// No pixels, no game (PLAN Phase 0 "Not in this phase").
fn run_placeholder_loop() -> anyhow::Result<()> {
    use std::io::Write;

    use crossterm::event::{Event, KeyCode, KeyEventKind};
    use crossterm::style::Print;
    use crossterm::{cursor, execute};

    let mut stdout = std::io::stdout();
    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        Print("BREAKOUT\n\nPhase 0 placeholder — no game yet.\n\nPress q to quit.\n")
    )?;
    stdout.flush()?;

    loop {
        // Clean shutdown on SIGINT/SIGTERM: the flag is set by caps.rs and
        // the guard drops normally (ADR-0010; the `drop(guard)` in main
        // restores the terminal).
        if term::caps::interrupted() {
            return Ok(());
        }

        // BREAKOUT_PANIC_TEST=1 proves the panic path restores the
        // terminal (ADR-0010, re-checked every phase).
        if std::env::var("BREAKOUT_PANIC_TEST").as_deref() == Ok("1") {
            panic!("forced panic inside the render loop (BREAKOUT_PANIC_TEST)");
        }

        if crossterm::event::poll(Duration::from_millis(20))? {
            if let Event::Key(key) = crossterm::event::read()? {
                if key.kind == KeyEventKind::Press
                    && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                {
                    return Ok(());
                }
            }
        }
    }
}
