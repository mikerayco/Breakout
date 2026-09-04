//! Terminal capability probing and the platform plumbing behind
//! `TerminalGuard` (ADR-0010).
//!
//! Owns: the graphics/keyboard/pixel probes (Phase 0), the signal-handler
//! installation, and the idempotent terminal teardown shared by the panic
//! hook and `TerminalGuard::drop`. This is one of two modules allowed
//! `unsafe` (PRD NFR-8); every `unsafe` block states the invariant it
//! relies on.

#![allow(unsafe_code)]

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;

/// Kitty graphics capability probe (query, direct transmit, 24-bit RGB):
/// the terminal must reply `_Gi=31;OK`.
const GRAPHICS_PROBE: &[u8] = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\";
const GRAPHICS_OK: &[u8] = b"_Gi=31;OK\x1b\\";
/// Keyboard enhancement capability query (ADR-0004); unsolicited reply is
/// `CSI ? <flags> u`.
const KEYBOARD_QUERY: &[u8] = b"\x1b[?u";

/// Set by the SIGINT/SIGTERM handler; polled by the main loop (ADR-0010).
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
/// True once teardown has run, so panic hook + Drop cannot double-restore.
static TEARDOWN_DONE: AtomicBool = AtomicBool::new(false);
/// True only while `TerminalGuard` owns the terminal; the panic hook must
/// not attempt teardown before (or after) a takeover.
static TAKEOVER_ACTIVE: AtomicBool = AtomicBool::new(false);

// --- Signals (ADR-0010) ------------------------------------------------------

extern "C" fn on_signal(_sig: libc::c_int) {
    // Invariant: this handler must be async-signal-safe; it only sets a flag
    // which the main loop polls, so the guard drops normally.
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Install SIGINT/SIGTERM handlers. Runs before the terminal is taken over.
fn install_signal_handlers() -> Result<()> {
    // SAFETY: sigaction/sigemptyset operate on a zero-initialised struct and
    // a handler that touches only an AtomicBool; no other shared mutable
    // state is touched. Failures are returned as errors, never panicked.
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = on_signal as *const () as usize;
        // SAFETY: sigemptyset does not fail.
        libc::sigemptyset(&mut sa.sa_mask);
        sa.sa_flags = 0; // no SA_SIGINFO: sa_sigaction is used as sa_handler
        for sig in [libc::SIGINT, libc::SIGTERM] {
            // SAFETY: valid sigaction pointer and null old-action pointer are
            // the documented contract; the return code is checked.
            let rc = libc::sigaction(sig, &sa, std::ptr::null_mut());
            if rc != 0 {
                anyhow::bail!("sigaction({sig}) failed with rc={rc}");
            }
        }
    }
    Ok(())
}

pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

// --- Teardown (ADR-0010) ------------------------------------------------------

/// Called by both `TerminalGuard::drop` and the panic hook. Idempotent and
/// infallible: every step is best-effort and continues past its own errors.
fn teardown() {
    let mut stdout = std::io::stdout();
    // Pop keyboard enhancement flags.
    let _ = write!(stdout, "\x1b[<u");
    // Delete all transmitted images, so nothing stays in scrollback.
    let _ = write!(stdout, "\x1b_Ga=d,d=A,q=2\x1b\\");
    // Show cursor, leave alternate screen, restore raw mode/echo.
    let _ = write!(stdout, "\x1b[?25h");
    let _ = write!(stdout, "\x1b[?1049l");
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = std::io::stdout().flush();
}

/// Idempotent, allocation-light teardown for the panic path and Drop.
/// No-ops unless a takeover is currently active (ADR-0010).
pub fn teardown_terminal_if_needed() {
    if !TAKEOVER_ACTIVE.load(Ordering::SeqCst) || TEARDOWN_DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    teardown();
    TAKEOVER_ACTIVE.store(false, Ordering::SeqCst);
}

/// Prepare signal handlers + flags before `TerminalGuard` takes over.
/// Returns Err without touching the terminal if (say) no tty is present.
pub fn prepare_for_takeover() -> Result<()> {
    TEARDOWN_DONE.store(false, Ordering::SeqCst);
    install_signal_handlers()?;
    Ok(())
}

pub(crate) fn mark_takeover_active() {
    TAKEOVER_ACTIVE.store(true, Ordering::SeqCst);
}

// --- Capability probe (FR-3 --caps, FR-7) -------------------------------------

/// Read from stdin until `stop` is found or `timeout` elapses, returning
/// everything read. Used to wait for the terminal's probe replies; never
/// blocks past the deadline even if the terminal never answers.
fn read_until(stop: &[u8], timeout: Duration) -> Vec<u8> {
    let deadline = Instant::now() + timeout;
    let mut buf = Vec::with_capacity(256);
    let mut chunk = [0u8; 128];
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining_ms = (deadline - now).as_millis().min(1_000) as libc::c_int;
        let mut pfd = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll() on stdin with a bounded timeout; pfd is a single
        // stack structure nobody else touches.
        let n = unsafe { libc::poll(&mut pfd, 1, remaining_ms) };
        if n <= 0 {
            continue; // timed out or interrupted; re-check the deadline
        }
        if pfd.revents & libc::POLLIN == 0 {
            continue;
        }
        // SAFETY: read into a fixed local buffer; the count is bounded by the
        // buffer length and read() fills exactly that many bytes at most.
        let got = unsafe { libc::read(libc::STDIN_FILENO, chunk.as_mut_ptr().cast(), chunk.len()) };
        if got <= 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..got as usize]);
        if buf.windows(stop.len()).any(|w| w == stop) {
            break;
        }
    }
    buf
}

/// Run `f` with stdin in raw (non-canonical) mode, then restore the
/// previous line discipline.
///
/// Probe replies (graphics `_Gi=31;OK`, keyboard `CSI ? u`) carry no
/// trailing newline; in canonical mode the tty driver would withhold them
/// until a line delimiter arrives, so the probe would time out. The kitty
/// examples read such replies with `stty raw -echo` for exactly this reason.
fn with_raw_stdin<R>(f: impl FnOnce() -> R) -> R {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return f(); // nothing to raw-ify; reads will see EOF (no reply)
    }
    // SAFETY: tcgetattr/tcsetattr on our controlling stdin; we snapshot and
    // restore the exact previous termios so the caller's terminal state is
    // untouched after the probe.
    let mut saved: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut saved) } != 0 {
        return f();
    }
    // Keep the untouched snapshot for restore; apply raw to a copy.
    let mut raw = saved;
    // SAFETY: cfmakeraw on a valid, recently-read termios; then applied with
    // TCSANOW (immediate), restoring the snapshot afterwards.
    unsafe {
        libc::cfmakeraw(&mut raw);
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw);
    }
    let result = f();
    // SAFETY: restore exactly what tcgetattr returned above.
    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &saved);
    }
    result
}

/// Ask the terminal whether shared-memory frames (`t=s`) actually
/// display. Ghostty's default image limits allow direct data (`t=d`) only,
/// answering `t=s` with `UnsupportedMedium` — and our per-frame `q=2`
/// suppresses that error, which used to mean a permanently empty screen.
/// A 1x1 probe with responses on (`q=0`) settles it once at startup; the
/// caller falls back to `t=d` for the session when this is false.
pub fn probe_shm() -> bool {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return false;
    }
    with_raw_stdin(|| {
        use std::io::Write;
        // 1x1 red pixel through the real shm path.
        let seg = match super::shm::transmit(&[255, 0, 0], 1, 1, 0) {
            Ok(seg) => seg,
            Err(_) => return false,
        };
        let mut stdout = std::io::stdout();
        let _ = write!(
            stdout,
            "\x1b_Ga=T,f=24,s=1,v=1,t=s,i=31,p=1,q=0;{}\x1b\\",
            seg.name_b64
        );
        let _ = stdout.flush();
        // Kitty answers `ESC _ G i=31;OK ESC \` on success, `;E...` on
        // rejection. Anything else (including silence) means no shm.
        let reply = read_until(b"\x1b\\", Duration::from_millis(300));
        let ok = reply.windows(4).any(|w| w == b";OK\x1b");
        // Delete the probe placement either way, then drop our segment
        // reference (the terminal unlinks it after a successful read).
        let _ = write!(stdout, "\x1b_Ga=d,d=I,i=31,q=2\x1b\\");
        let _ = stdout.flush();
        super::shm::unlink_b64(&seg.name_b64);
        ok
    })
}

/// Ask the terminal whether it implements the Kitty graphics protocol.
pub fn probe_graphics() -> bool {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        // No terminal to answer; nothing to probe (no query bytes leaked).
        return false;
    }
    with_raw_stdin(|| {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(GRAPHICS_PROBE);
        let _ = stdout.flush();
        read_until(GRAPHICS_OK, Duration::from_millis(200))
            .windows(GRAPHICS_OK.len())
            .any(|w| w == GRAPHICS_OK)
    })
}

/// Ask for the keyboard enhancement flags (ADR-0004). Reply is
/// `CSI ? <flags> u`; returns the flags integer.
pub fn probe_keyboard_flags() -> Option<u64> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return None;
    }
    with_raw_stdin(|| {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(KEYBOARD_QUERY);
        let _ = stdout.flush();
        let reply = read_until(b"u", Duration::from_millis(200));
        let s = String::from_utf8_lossy(&reply);
        let marker = "\x1b[?";
        let rest = s.split_once(marker).map(|(_, r)| r)?;
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok()
    })
}

/// Pixel geometry from TIOCGWINSZ + CSI fallbacks (ADR-0003).
/// Each pair is (width, height) in pixels; either may be unknown.
#[derive(Debug, Clone, Copy, Default)]
pub struct PixelGeometry {
    pub cell: Option<(u32, u32)>,
    pub window: Option<(u32, u32)>,
}

pub fn pixel_geometry() -> PixelGeometry {
    unsafe {
        // SAFETY: TIOCGWINSZ copies out into an in-place winsize struct; the
        // pointer is valid for the lifetime of ws, and all fields are read
        // only after the return code says the struct was filled.
        let mut ws: libc::winsize = std::mem::zeroed();
        let rc = libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws);
        if rc == 0 {
            let window = if ws.ws_xpixel > 0 && ws.ws_ypixel > 0 {
                Some((ws.ws_xpixel as u32, ws.ws_ypixel as u32))
            } else {
                None
            };
            let cell = if window.is_some() && ws.ws_col > 0 && ws.ws_row > 0 {
                Some((
                    ws.ws_xpixel as u32 / ws.ws_col as u32,
                    ws.ws_ypixel as u32 / ws.ws_row as u32,
                ))
            } else {
                None
            };
            return PixelGeometry { cell, window };
        }
    }
    PixelGeometry::default()
}

/// Terminal identity from the environment (FR-7: cached for the session).
pub fn terminal_identity() -> (String, String) {
    let program = std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "unknown".into());
    let term = std::env::var("TERM").unwrap_or_else(|_| "unknown".into());
    (program, term)
}

/// The capability report printed by `--caps` (FR-3).
pub fn capability_report() -> String {
    let (program, term) = terminal_identity();
    let graphics = probe_graphics();
    let keyboard = probe_keyboard_flags();
    let geometry = pixel_geometry();
    let cell_str = geometry
        .cell
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|| "unknown".into());
    let window_str = geometry
        .window
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|| "unknown".into());

    format!(
        "breakout capability report\n\
         terminal program: {program}\n\
         TERM:                {term}\n\
         graphics protocol:  {}\n\
         keyboard protocol:  {}\n\
         cell pixel size:    {cell_str}\n\
         window pixel size:  {window_str}\n         audio:               {}\n",
        if graphics { "kitty (ok)" } else { "MISSING" },
        match keyboard {
            Some(flags) => format!("kitty (query ok, current state flags={flags})"),
            None => "not supported (degraded input mode)".to_string(),
        },
        crate::audio::probe_summary(),
    )
}
