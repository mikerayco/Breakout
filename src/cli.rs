//! CLI flag parsing — the flag set in PRD FR-3 plus `--no-bloom` (FR-29).

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "breakout",
    version,
    about = "GPU-accelerated Breakout for graphics-capable terminals",
    long_about = None
)]
pub struct Cli {
    /// Load one .lvl file and play it standalone, outside the run structure
    /// (FR-2). Single file only; a directory is not accepted (OQ-6 resolved).
    #[arg(long, value_name = "PATH")]
    pub level: Option<PathBuf>,

    /// Disable the audio subsystem entirely for this session (FR-48).
    #[arg(long)]
    pub no_audio: bool,

    /// Disable the bloom pass for this session (FR-29). Also toggled live
    /// with `F4`.
    #[arg(long)]
    pub no_bloom: bool,

    /// Presentation rate, 30-144. Frames are dropped, never queued (FR-9).
    #[arg(long, value_name = "N", default_value_t = 60, value_parser = clap::value_parser!(u32).range(30..=144))]
    pub fps: u32,

    /// Force an integer scale factor; default is auto (ADR-0003).
    #[arg(long, value_name = "N")]
    pub scale: Option<u32>,

    /// Fix the RNG seed for a reproducible run (FR-39, NFR-10).
    #[arg(long, value_name = "N")]
    pub seed: Option<u64>,

    /// Wipe persistent progression after an explicit y/N confirmation (FR-45).
    #[arg(long)]
    pub reset_profile: bool,

    /// Print the terminal capability report and exit 0 (FR-3).
    #[arg(long)]
    pub caps: bool,
}
