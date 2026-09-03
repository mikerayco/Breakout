//! Breakout — a GPU-accelerated terminal arcade game (PRD §1, ADR-0001).
//!
//! Startup path: parse flags (FR-3), probe capabilities, take over the
//! terminal via `TerminalGuard` (ADR-0010), then run. Every exit path —
//! normal, panic, SIGINT, SIGTERM — restores the terminal.

#![deny(unsafe_code)] // NFR-8: unsafe lives only in term/shm.rs and term/caps.rs

mod cli;
mod term;

#[allow(dead_code)]
mod audio;
mod game;
#[allow(dead_code)]
mod save;

mod render;

use render::{compute_scale, Framebuffer};

use std::time::{Duration, Instant};

use clap::Parser;

use crate::cli::Cli;
use crate::game::physics::World;
use crate::game::state::GameState;

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

    // --reset-profile arrives in Phase 8; refuse cleanly until then.
    if cli.reset_profile {
        eprintln!("--reset-profile arrives with the save profile in Phase 8.");
        std::process::exit(2);
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

/// Phase 3 game loop: protocol held-key input with legacy fallback
/// (ADR-0004), fixed-timestep simulation (ADR-0006), pause menu (FR-6),
/// mute + debug toggles, resize handling (FR-10/11), double-buffered
/// transport (ADR-0002).
fn run_loop(cli: Cli) -> anyhow::Result<()> {
    use crossterm::event::Event;
    use game::{
        physics::{InputState, RunModifiers},
        tuning,
    };
    use render::Frames;
    use term::input::{Action, Poller};

    let transport = term::kgp::Transport::detect();
    let mut frames = Frames::new(cli.fps, transport);

    let seed = cli.seed.unwrap_or(0xBEEF);
    let mut world = World::new(
        game::level::default_bricks(),
        seed,
        0,
        RunModifiers::default(),
    );
    world.state = GameState::Title;

    let mut fb = current_framebuffer(cli.scale);

    // Resolve the input mode once (probe reads stdin; never mid-frame).
    let mut poller = Poller::new(term::input::legacy_mode());
    let mut launch_edge = false;
    let mut muted = cli.no_audio;
    let mut debug = false;
    let mut pause_selected: usize = 0;
    // NFR-2 readout: last measured key-to-movement latency.
    let mut latency_ms: Option<f32> = None;

    let mut accumulator = 0.0f32;
    let mut last_frame = Instant::now();

    loop {
        if term::caps::interrupted() {
            return Ok(());
        }
        if std::env::var("BREAKOUT_PANIC_TEST").as_deref() == Ok("1") {
            panic!("forced panic inside the render loop (BREAKOUT_PANIC_TEST)");
        }

        // Drain input without blocking past the next frame deadline.
        let deadline = frames.wait_until_next();
        let poll_start = Instant::now();
        while crossterm::event::poll(Duration::from_secs(0))? {
            match crossterm::event::read()? {
                Event::Key(key) => poller.observe(&key, Instant::now()),
                // Focus loss / unexpected gap: never leave the paddle stuck.
                Event::FocusLost => poller.clear_held(),
                _ => {}
            }
            if poll_start.elapsed() > Duration::from_millis(4) {
                break;
            }
        }

        // Edge actions from this frame.
        for action in poller.take_edges() {
            match action {
                Action::Quit => return Ok(()),
                Action::Pause => match world.state {
                    GameState::Playing => {
                        world.state = GameState::Paused;
                        pause_selected = 0;
                    }
                    GameState::Paused => {
                        world.state = GameState::Playing;
                    }
                    _ => return Ok(()),
                },
                Action::Launch => {
                    if world.state == GameState::Playing {
                        launch_edge = true;
                    } else if world.state == GameState::Paused {
                        if activate_pause_item(
                            &mut world,
                            &mut pause_selected,
                            &mut muted,
                            cli.seed,
                        ) {
                            return Ok(());
                        }
                    } else {
                        advance_state(&mut world);
                    }
                }
                Action::MenuConfirm => {
                    if world.state == GameState::Paused {
                        if activate_pause_item(
                            &mut world,
                            &mut pause_selected,
                            &mut muted,
                            cli.seed,
                        ) {
                            return Ok(());
                        }
                    } else if world.state != GameState::Playing {
                        advance_state(&mut world);
                    } else {
                        launch_edge = true;
                    }
                }
                Action::MenuUp => {
                    if world.state == GameState::Paused {
                        pause_selected = pause_selected.saturating_sub(1);
                    }
                }
                Action::MenuDown => {
                    if world.state == GameState::Paused {
                        pause_selected = (pause_selected + 1).min(3);
                    }
                }
                Action::Mute => {
                    muted = !muted;
                }
                Action::Debug => {
                    debug = !debug;
                }
            }
        }
        // Quit returns above; Esc on a non-playing, non-paused menu quits
        // via the Pause arm. Space/Enter advances menus via Launch/Confirm.

        let now = Instant::now();
        if now < frames.next_due() {
            std::thread::sleep(deadline.min(Duration::from_millis(2)));
            continue;
        }
        frames.record_presented(now);

        // Fixed-timestep accumulator (ADR-0006).
        let elapsed = (now - last_frame).as_secs_f32().min(0.25);
        last_frame = now;
        if world.state == GameState::Playing {
            let held = poller.held(now);
            // NFR-2 probe: if a movement key was pressed and the paddle is
            // now moving, the key-to-movement delay is (now - press).
            if let Some(t) = poller.last_press {
                if (held.left || held.right) && world.paddle_vel != 0.0 {
                    latency_ms = Some((now - t).as_secs_f32() * 1000.0);
                }
            }
            accumulator += elapsed;
            let mut steps = 0u8;
            while accumulator >= tuning::DT && steps < tuning::MAX_CATCHUP {
                let input = InputState {
                    left: held.left,
                    right: held.right,
                    launch: launch_edge,
                };
                world.step(input, tuning::DT);
                accumulator -= tuning::DT;
                steps += 1;
                launch_edge = false;
                if world.state != GameState::Playing {
                    accumulator = 0.0;
                    break;
                }
            }
            if accumulator >= tuning::DT {
                accumulator = 0.0;
            }
        } else {
            accumulator = 0.0;
            launch_edge = false;
        }

        // Resize (FR-10), live recovery from too-small (FR-11).
        match current_framebuffer(cli.scale) {
            Some(wanted) if wanted.scale() != fb.as_ref().map(Framebuffer::scale).unwrap_or(0) => {
                resize_rebuild(&mut fb, wanted, &mut frames);
            }
            None => {
                if let Some(cur) = &mut fb {
                    render::draw::draw_too_small(cur);
                }
            }
            _ => {}
        }

        if let Some(cur) = &mut fb {
            render::draw::draw_world(cur, &world, muted);
            if world.state == GameState::Paused {
                render::draw::draw_pause_menu(cur, pause_selected, muted);
            }
            let (p50, p99) = frames.percentiles();
            let mut line = format!(
                "FPS {:>3.0} P50 {:>4.1} P99 {:>4.1} {} S={}",
                frames.avg_fps(),
                p50,
                p99,
                frames.transport.name(),
                cur.scale(),
            );
            if debug {
                let held = poller.held(now);
                line = format!(
                    "{line} IN={} HELD={}{} LAT={} VEL={:.0} BALLS={} MUTE={}",
                    if poller.is_legacy() {
                        "legacy"
                    } else {
                        "kitty"
                    },
                    if held.left { "L" } else { "-" },
                    if held.right { "R" } else { "-" },
                    latency_ms.map_or("--".to_string(), |v| format!("{v:.1}MS")),
                    world.paddle_vel,
                    world.balls.len(),
                    if muted { "ON" } else { "OFF" },
                );
            }
            render::draw::draw_fps_line(cur, &line);
            let (id, prev) = frames.next_image_id();
            let w = cur.width();
            let h = cur.height();
            let rgb = cur.rgb_bytes();
            term::kgp::send_frame(frames.transport, rgb, w, h, id, prev)?;
        }
    }
}

/// One pause-menu activation (FR-6): Resume / Restart run / Mute / Quit.
/// Returns true when the player chose Quit (exit to shell).
fn activate_pause_item(
    world: &mut World,
    selected: &mut usize,
    muted: &mut bool,
    seed_opt: Option<u64>,
) -> bool {
    use game::state::StateEvent;
    match *selected % 4 {
        0 => {
            world.state = world
                .state
                .transition(StateEvent::Resume)
                .unwrap_or(GameState::Playing);
            false
        }
        1 => {
            *world = World::new(
                game::level::default_bricks(),
                seed_opt.unwrap_or(0xBEEF),
                0,
                world.modifiers,
            );
            false
        }
        2 => {
            *muted = !*muted;
            false
        }
        _ => true,
    }
}

/// Space/Enter on a menu state: title -> play, clear -> next level (fresh
/// bricks for Phase 2), over -> fresh run.
fn advance_state(world: &mut World) {
    use game::state::StateEvent;
    match world.state {
        GameState::Title => {
            world.state = world
                .state
                .transition(StateEvent::Start)
                .unwrap_or(GameState::Playing);
        }
        GameState::LevelClear => {
            *world = World::new(
                game::level::default_bricks(),
                world.rng.next_u32_below(u32::MAX) as u64,
                world.level_index.saturating_add(1),
                world.modifiers,
            );
        }
        GameState::RunOver => {
            *world = World::new(
                game::level::default_bricks(),
                world.rng.next_u32_below(u32::MAX) as u64,
                0,
                world.modifiers,
            );
            world.state = GameState::Title;
        }
        GameState::Paused => {
            world.state = world
                .state
                .transition(StateEvent::Resume)
                .unwrap_or(GameState::Playing);
        }
        GameState::Playing => {}
    }
}

/// Build the framebuffer at the current scale, or `None` if the window is
/// too small for scale 1 (FR-11).
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
                None => return Some(Framebuffer::new(1).expect("scale-1 framebuffer")),
            }
        }
    };
    Framebuffer::new(scale)
}

/// Reallocate the framebuffer on a scale change (FR-10). Deletes all images.
fn resize_rebuild(fb: &mut Option<Framebuffer>, new_fb: Framebuffer, frames: &mut render::Frames) {
    use std::io::Write;
    let _ = write!(std::io::stdout(), "\x1b_Ga=d,d=A,q=2\x1b\\");
    let _ = std::io::stdout().flush();
    *fb = Some(new_fb);
    frames.image_id = 1;
}
