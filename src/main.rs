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

    // Phase 5 developer path: validate a level directory without
    // launching the game (no graphics probe, no terminal takeover).
    if let Some(dir) = &cli.validate {
        return validate_levels(dir);
    }

    // --reset-profile arrives in Phase 8; refuse cleanly until then.
    if cli.reset_profile {
        eprintln!("--reset-profile arrives with the save profile in Phase 8.");
        std::process::exit(2);
    }

    // FR-38: --level loads one file from disk and plays it standalone.
    // Parsed before the graphics probe so a malformed level reports its
    // precise error (FR-36) without needing a graphics terminal, and the
    // terminal is never taken over for a content error.
    let startup = match &cli.level {
        None => StartupLevel::default_run(),
        Some(path) => match game::level::parse_file(path) {
            Ok(lvl) => StartupLevel::from_level(&lvl),
            Err(e) => {
                eprintln!("breakout: {e}");
                std::process::exit(1);
            }
        },
    };

    // FR-4: graphics support is mandatory, no text fallback.
    if !term::caps::probe_graphics() {
        eprintln!("{FR4_GRAPHICS_MISSING}");
        std::process::exit(2);
    }

    install_panic_hook();
    let guard = term::guard::TerminalGuard::enter()?;

    run_loop(cli, startup)?;

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

/// What the loop plays first: the default hard-coded level, or one
/// `--level` file standalone (FR-38). Per-level knobs ride the
/// data-driven modifier axes (ADR-0008): no special cases in physics.
struct StartupLevel {
    bricks: Vec<game::physics::Brick>,
    level_index: u32,
    modifiers: game::physics::RunModifiers,
}

impl StartupLevel {
    /// Default: the hard-coded level at index 0 (campaign runs, Phase 8).
    fn default_run() -> Self {
        Self {
            bricks: game::level::default_bricks(),
            level_index: 0,
            modifiers: game::physics::RunModifiers::default(),
        }
    }

    /// One `--level` file: tier selects the speed ramp, header knobs map
    /// onto modifier axes (ball speed, drop-rate delta).
    fn from_level(lvl: &game::level::Level) -> Self {
        Self {
            bricks: lvl.bricks.clone(),
            level_index: u32::from(lvl.tier.saturating_sub(1)),
            modifiers: game::physics::RunModifiers {
                ball_speed_mul: lvl.ball_speed,
                drop_rate_add: lvl.drop_rate - game::tuning::DROP_RATE_DEFAULT,
                ..Default::default()
            },
        }
    }
}

/// `--validate <dir>`: parse every `.lvl` in the directory, report one
/// line per file plus a summary, and exit without launching the game.
fn validate_levels(dir: &std::path::Path) -> anyhow::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("cannot list {}: {e}", dir.display()))?
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "lvl").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    let mut ok = 0u32;
    let mut fail = 0u32;
    for entry in entries {
        let path = entry.path();
        match game::level::parse_file(&path) {
            Ok(lvl) => {
                println!(
                    "OK {} (tier {}, {} bricks)",
                    path.display(),
                    lvl.tier,
                    lvl.bricks.len()
                );
                ok += 1;
            }
            Err(e) => {
                println!("FAIL {e}");
                fail += 1;
            }
        }
    }
    println!("{ok} valid, {fail} invalid");
    if fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Game loop: protocol held-key input with legacy fallback (ADR-0004),
/// fixed-timestep simulation (ADR-0006), pause menu (FR-6), juice (Phase 4),
/// resize handling (FR-10/11), double-buffered transport (ADR-0002).
fn run_loop(cli: Cli, startup: StartupLevel) -> anyhow::Result<()> {
    use crossterm::event::Event;
    use game::{
        physics::{BrickKind, InputState},
        tuning,
    };
    use render::draw::JuiceView;
    use render::Frames;
    use term::input::{Action, Poller};

    let transport = term::kgp::Transport::detect();
    let mut frames = Frames::new(cli.fps, transport);

    let seed = cli.seed.unwrap_or(0xBEEF);
    let mut world = World::new(startup.bricks, seed, startup.level_index, startup.modifiers);
    world.state = GameState::Title;

    let mut fb = current_framebuffer(cli.scale);

    // Resolve the input mode once (probe reads stdin; never mid-frame).
    let mut poller = Poller::new(term::input::legacy_mode());
    let mut launch_edge = false;
    let mut muted = cli.no_audio;
    let mut debug = false;
    let mut bloom_on = !cli.no_bloom;
    let mut pause_selected: usize = 0;
    // NFR-2 readout: last measured key-to-movement latency.
    let mut latency_ms: Option<f32> = None;

    // Phase 4 juice state (all allocated once, reused every frame — no
    // per-frame allocation on the hot path).
    let mut pool = render::particles::Pool::new();
    let mut shake = render::camera::Shake::new();
    let mut vrng = fastrand::Rng::with_seed(seed ^ 0x9E3779B9);
    let mut trails: Vec<std::collections::VecDeque<(f32, f32)>> = Vec::new();
    let mut brick_flashes: Vec<(u8, u8)> = Vec::new();
    let mut rings: Vec<(f32, f32, f32)> = Vec::new();
    let mut prev_bricks: Vec<(u8, u8, u8, BrickKind)> = Vec::new();
    let mut prev_lives = world.lives;
    let mut prev_combo = 0u32;
    let mut prev_run: u64 = 0;
    let mut run_id: u64 = 0;
    let mut hit_stop = 0.0f32;
    let mut paddle_flash_t = 0.0f32;
    let mut combo_pop_t = 0.0f32;

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
                            &mut run_id,
                        ) {
                            return Ok(());
                        }
                    } else {
                        advance_state(&mut world, &mut run_id);
                    }
                }
                Action::MenuConfirm => {
                    if world.state == GameState::Paused {
                        if activate_pause_item(
                            &mut world,
                            &mut pause_selected,
                            &mut muted,
                            cli.seed,
                            &mut run_id,
                        ) {
                            return Ok(());
                        }
                    } else if world.state != GameState::Playing {
                        advance_state(&mut world, &mut run_id);
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
                Action::Bloom => {
                    bloom_on = !bloom_on;
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

        // Fixed-timestep accumulator (ADR-0006). Hit-stop (FR-26) freezes
        // the simulation while rendering continues: the accumulator is
        // consumed without stepping.
        let elapsed = (now - last_frame).as_secs_f32().min(0.25);
        last_frame = now;
        if hit_stop > 0.0 {
            hit_stop -= elapsed;
            accumulator = 0.0;
            launch_edge = false;
        }
        if world.state == GameState::Playing && hit_stop <= 0.0 {
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
        } else if world.state != GameState::Playing {
            accumulator = 0.0;
            launch_edge = false;
        }

        // Juice events: diff the sim bricks against last frame (no
        // allocation — the buffers are reused). A changed run id means a
        // fresh level, so snapshot quietly instead of bursting.
        if run_id != prev_run {
            prev_run = run_id;
            pool = render::particles::Pool::new();
            trails.clear();
            rings.clear();
            brick_flashes.clear();
            hit_stop = 0.0;
            paddle_flash_t = 0.0;
            combo_pop_t = 0.0;
            snapshot_bricks(&world, &mut prev_bricks);
            prev_lives = world.lives;
            prev_combo = world.score.combo;
        } else if hit_stop <= 0.0 {
            poll_juice_events(
                &world,
                &mut prev_bricks,
                &mut prev_lives,
                &mut prev_combo,
                &mut pool,
                &mut vrng,
                &mut shake,
                &mut rings,
                &mut brick_flashes,
                &mut hit_stop,
                &mut paddle_flash_t,
                &mut combo_pop_t,
            );
        }
        // Cosmetic timers always advance on render time (even in hit-stop).
        paddle_flash_t = (paddle_flash_t - elapsed).max(0.0);
        combo_pop_t = (combo_pop_t - elapsed).max(0.0);
        for ring in &mut rings {
            ring.2 = (ring.2 + elapsed / render::draw::COMBO_POP_SECS).min(1.0);
        }
        rings.retain(|r| r.2 < 1.0);
        pool.update(elapsed);
        // Camera offset for this frame (decays on render time).
        let (ox, oy) = shake.offset(&mut vrng, elapsed);
        // Ball trails: ring buffers of past positions (FR-27).
        if trails.len() != world.balls.len() {
            trails.resize_with(world.balls.len(), Default::default);
        }
        for (hist, ball) in trails.iter_mut().zip(world.balls.iter()) {
            if ball.stuck {
                hist.clear();
            } else {
                hist.push_back((ball.x, ball.y));
                while hist.len() > render::draw::TRAIL_N {
                    hist.pop_front();
                }
            }
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
            let jv = JuiceView {
                ox,
                oy,
                brick_flashes: &brick_flashes,
                paddle_flash: paddle_flash_t > 0.0,
                rings: &rings,
                combo_pop: combo_pop_t > 0.0,
            };
            render::draw::draw_world(cur, &world, muted, &jv);
            for (hist, ball) in trails.iter().zip(world.balls.iter()) {
                if hist.len() > 1 && !ball.stuck {
                    render::draw::draw_trail(cur, hist, ox, oy);
                }
            }
            let pscale = cur.scale() as f32;
            pool.draw(cur.pixmap_mut(), pscale, ox, oy);
            if bloom_on {
                render::bloom::apply(cur.pixmap_mut());
            }
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
                    "{line} IN={} HELD={}{} LAT={} VEL={:.0} BALLS={} MUTE={} BLOOM={} P={} SHK={:.1}",
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
                    if bloom_on { "ON" } else { "OFF" },
                    pool.len(),
                    shake.magnitude(),
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

/// Snapshot the brick grid into a reusable buffer (no allocation after
/// the first frames once capacity settles).
fn snapshot_bricks(world: &World, out: &mut Vec<(u8, u8, u8, game::physics::BrickKind)>) {
    out.clear();
    out.extend(world.bricks.iter().map(|b| (b.col, b.row, b.hp, b.kind)));
}

/// Diff last frame's bricks against this frame's and fire juice: bursts,
/// explosions, rings, flashes, shake and hit-stop. All buffers are owned
/// by the caller and reused (no per-frame allocation).
#[allow(clippy::too_many_arguments)]
fn poll_juice_events(
    world: &World,
    prev_bricks: &mut Vec<(u8, u8, u8, game::physics::BrickKind)>,
    prev_lives: &mut i32,
    prev_combo: &mut u32,
    pool: &mut render::particles::Pool,
    vrng: &mut fastrand::Rng,
    shake: &mut render::camera::Shake,
    rings: &mut Vec<(f32, f32, f32)>,
    brick_flashes: &mut Vec<(u8, u8)>,
    hit_stop: &mut f32,
    paddle_flash_t: &mut f32,
    combo_pop_t: &mut f32,
) {
    use game::physics::BrickKind;
    use game::tuning;
    use render::{camera, draw, particles};

    brick_flashes.clear();
    for (pc, pr, php, pkind) in prev_bricks.iter().copied() {
        match world.bricks.iter().find(|b| b.col == pc && b.row == pr) {
            None => {
                // Destroyed this frame.
                let cx = tuning::GRID_ORIGIN_X
                    + f32::from(pc) * tuning::BRICK_CELL_W
                    + tuning::BRICK_DRAW_W / 2.0;
                let cy = tuning::GRID_ORIGIN_Y
                    + f32::from(pr) * tuning::BRICK_CELL_H
                    + tuning::BRICK_DRAW_H / 2.0;
                match pkind {
                    BrickKind::Steel => {}
                    BrickKind::Normal => {
                        let tmp = game::physics::Brick::normal(pc, pr, php);
                        let n = vrng.u32(particles::BURST_N_MIN..=particles::BURST_N_MAX);
                        pool.burst(&mut *vrng, cx, cy, draw::brick_rgb(&tmp), n);
                        shake.add(camera::SHAKE_BRICK);
                    }
                    BrickKind::Explosive => {
                        let tmp = game::physics::Brick::explosive(pc, pr);
                        pool.explosion(&mut *vrng, cx, cy, draw::brick_rgb(&tmp));
                        if rings.len() < 16 {
                            rings.push((cx, cy, 0.0));
                        }
                        shake.add(camera::SHAKE_EXPLOSION);
                    }
                }
                if pkind != BrickKind::Steel {
                    *hit_stop = hit_stop.max(tuning::hit_stop_secs(world.score.combo));
                }
            }
            Some(cur) => {
                if cur.hp < php {
                    brick_flashes.push((pc, pr));
                }
            }
        }
    }
    if world.lives < *prev_lives {
        shake.add(camera::SHAKE_LIFE_LOST);
    }
    if world.score.combo > *prev_combo {
        *combo_pop_t = draw::COMBO_POP_SECS;
    }
    if world.score.combo == 0 && *prev_combo > 0 && !world.balls.is_empty() {
        *paddle_flash_t = draw::PADDLE_FLASH_SECS;
    }
    snapshot_bricks(world, prev_bricks);
    *prev_lives = world.lives;
    *prev_combo = world.score.combo;
}

/// One pause-menu activation (FR-6): Resume / Restart run / Mute / Quit.
/// Returns true when the player chose Quit (exit to shell).
fn activate_pause_item(
    world: &mut World,
    selected: &mut usize,
    muted: &mut bool,
    seed_opt: Option<u64>,
    run_id: &mut u64,
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
            *run_id += 1;
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
fn advance_state(world: &mut World, run_id: &mut u64) {
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
            *run_id += 1;
        }
        GameState::RunOver => {
            *world = World::new(
                game::level::default_bricks(),
                world.rng.next_u32_below(u32::MAX) as u64,
                0,
                world.modifiers,
            );
            world.state = GameState::Title;
            *run_id += 1;
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
