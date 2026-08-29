//! Breakout — a GPU-accelerated terminal arcade game (PRD §1, ADR-0001).
//!
//! Startup path: parse flags (FR-3), probe capabilities, take over the
//! terminal via `TerminalGuard` (ADR-0010), then run. Every exit path —
//! normal, panic, SIGINT, SIGTERM — restores the terminal.

#![deny(unsafe_code)] // NFR-8: unsafe lives only in term/shm.rs and term/caps.rs

mod cli;
mod term;

// Skeleton modules from ADR-0005 §Layout, compiling with stub bodies until
// their phases (audio: 7, save: 8).
#[allow(dead_code)]
mod audio;
#[allow(dead_code)]
mod game;
#[allow(dead_code)]
mod save;

mod render;

use render::{compute_scale, Framebuffer};

use std::time::Instant;

use clap::Parser;

use crate::cli::Cli;

const FR4_GRAPHICS_MISSING: &str = "\
This terminal does not support the Kitty graphics protocol, which breakout requires.

Known-working terminals: Ghostty, Kitty, WezTerm.";

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // FR-3: --caps prints the capability report and exits 0.
    if cli.caps {
        print!("{}", term::caps::capability_report());
        return Ok(());
    }

    // FR-4: graphics support is mandatory, no text fallback.
    if !term::caps::probe_graphics() {
        eprintln!("{FR4_GRAPHICS_MISSING}");
        std::process::exit(2);
    }

    install_panic_hook();
    let guard = term::guard::TerminalGuard::enter()?;

    run_loop(cli)?;

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

/// Phase 1 render loop: animated test card at `--fps`, with resize handling
/// and the double-buffered frame transport (PLAN Phase 1).
fn run_loop(cli: Cli) -> anyhow::Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEventKind};
    use render::Frames;

    let transport = term::kgp::Transport::detect();
    let mut frames = Frames::new(cli.fps, transport);

    // Framebuffer at the current scale; None when the window is too small
    // for scale 1 (FR-11).
    let mut fb = current_framebuffer(cli.scale);
    let mut card = render::draw::TestCard::new(fb.as_ref().map(Framebuffer::scale).unwrap_or(1));

    let mut last_tick = Instant::now();

    loop {
        // Clean shutdown on SIGINT/SIGTERM (ADR-0010).
        if term::caps::interrupted() {
            return Ok(());
        }

        // BREAKOUT_PANIC_TEST=1: prove the panic path restores the terminal
        // (ADR-0010, re-checked every phase).
        if std::env::var("BREAKOUT_PANIC_TEST").as_deref() == Ok("1") {
            panic!("forced panic inside the render loop (BREAKOUT_PANIC_TEST)");
        }

        // Input: q/Esc quits; everything else is ignored in Phase 1.
        if crossterm::event::poll(frames.wait_until_next())? {
            if let Event::Key(key) = crossterm::event::read()? {
                if key.kind == KeyEventKind::Press
                    && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                {
                    return Ok(());
                }
            }
        }

        // Present when due; if we're late the clock snaps — frames are
        // dropped, never queued (FR-9).
        let now = Instant::now();
        if now < frames.next_due() {
            continue;
        }
        frames.record_presented(now);

        card.tick((now - last_tick).as_secs_f32().min(0.25));
        last_tick = now;

        // Resize: recompute the scale, reallocate if it changed (FR-10), and
        // recover live from a too-small window (FR-11).
        let wanted = current_framebuffer(cli.scale);
        match wanted {
            Some(wanted) if wanted.scale() != fb.as_ref().map(Framebuffer::scale).unwrap_or(0) => {
                // Scale changed (or we just recovered from too-small).
                resize_rebuild(&mut fb, wanted, &mut card, &mut frames);
            }
            None => {
                // Too small for scale 1: show the message, keep rendering.
                if let Some(cur) = &mut fb {
                    render::draw::draw_too_small(cur);
                }
            }
            _ => {} // scale unchanged
        }

        // Animate + overlay (only when we have a full-size framebuffer).
        if let Some(cur) = &mut fb {
            card.draw(cur);
            let (p50, p99) = frames.percentiles();
            card.draw_overlay(cur, frames.avg_fps(), p50, p99, frames.transport.name());

            // Present via transport; double-buffer image ids (ADR-0002).
            let (id, prev) = frames.next_image_id();
            let w = cur.width();
            let h = cur.height();
            let rgb = cur.rgb_bytes();
            term::kgp::send_frame(frames.transport, rgb, w, h, id, prev)?;
        }
    }
}

/// Build the framebuffer at the current scale, or `None` if the window is
/// too small for scale 1 (FR-11). `--scale` overrides the auto computation
/// for testing (ADR-0003).
fn current_framebuffer(forced_scale: Option<u32>) -> Option<Framebuffer> {
    let scale = match forced_scale {
        Some(s) => s.clamp(
            render::framebuffer::MIN_SCALE,
            render::framebuffer::MAX_SCALE,
        ),
        None => {
            let geo = term::caps::pixel_geometry();
            match geo.window {
                Some((w, h)) => compute_scale(w, h)?,
                // No pixel size available (non-Ghostty terminal); fall back
                // to scale 1 so the test card still renders somewhere.
                None => return Some(Framebuffer::new(1).expect("scale-1 framebuffer")),
            }
        }
    };
    Framebuffer::new(scale)
}

/// Reallocate the framebuffer + card on a scale change (FR-10). Deletes all
/// images; the next transmit starts with a clean id.
fn resize_rebuild(
    fb: &mut Option<Framebuffer>,
    new_fb: Framebuffer,
    card: &mut render::draw::TestCard,
    frames: &mut render::Frames,
) {
    use std::io::Write;
    // Delete every image placement; the next frame starts clean.
    let _ = write!(std::io::stdout(), "\x1b_Ga=d,d=A,q=2\x1b\\");
    let _ = std::io::stdout().flush();
    *fb = Some(new_fb);
    card.scale = fb.as_ref().map(Framebuffer::scale).unwrap_or(1);
    frames.image_id = 1;
}
