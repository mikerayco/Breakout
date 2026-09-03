//! `.lvl` parsing and the brick grid (ADR-0007).
//!
//! Owns the TOML-header + ASCII-grid parser with path/line/column/char
//! errors (FR-36) and the in-memory brick grid. Never panics on any input
//! (property-tested). The campaign levels are compiled in via the
//! `build.rs`-generated manifest; `--level` reads from disk instead.

use std::fmt;
use std::path::Path;

use super::physics::Brick;
use super::tuning;

/// Grid width in cells (ADR-0007: exactly 18 columns).
pub const GRID_W: usize = 18;
/// Grid height cap in rows (ADR-0007: 1-14 rows).
pub const GRID_ROWS_MAX: usize = 14;
/// Tier range (ADR-0007 rule 5, ADR-0008 pools).
pub const TIER_MIN: u8 = 1;
/// Tier range.
pub const TIER_MAX: u8 = 4;

// Build-generated: (&str id, &str source) per campaign level, sorted NN-.
include!(concat!(env!("OUT_DIR"), "/campaign_manifest.rs"));

/// One parsed level: header knobs plus the brick grid.
#[derive(Debug, Clone)]
pub struct Level {
    /// Display name (header `name`).
    pub name: String,
    /// Difficulty pool 1-4 (header `tier`).
    pub tier: u8,
    /// Ball speed multiplier (header `ball_speed`, default 1.0).
    pub ball_speed: f32,
    /// Powerup drop chance per brick (header `drop_rate`, default tuning).
    pub drop_rate: f32,
    /// Palette id (header `palette`, default "neon").
    pub palette: String,
    /// Bricks from the grid.
    pub bricks: Vec<Brick>,
}

/// Precise parse failure: file path, line, column and message (FR-36).
/// Grid line numbers count the `---` separator as line 1, so the first
/// grid row is line 2; columns are 1-indexed cells/characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelError {
    /// File path (or label) as given.
    pub path: String,
    /// Line number (separator-relative for grid errors, 0 for file errors).
    pub line: u32,
    /// Column number, 1-indexed (0 for file/line errors).
    pub column: u32,
    /// What is wrong (names the offending character where there is one).
    pub message: String,
}

impl LevelError {
    fn at(path: &str, line: u32, column: u32, message: String) -> Self {
        Self {
            path: path.to_string(),
            line,
            column,
            message,
        }
    }
}

impl fmt::Display for LevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.path, self.message)
        } else if self.column == 0 {
            write!(f, "{}: line {}: {}", self.path, self.line, self.message)
        } else {
            write!(
                f,
                "{}: line {}, column {}: {}",
                self.path, self.line, self.column, self.message
            )
        }
    }
}

impl std::error::Error for LevelError {}

/// Parse one `.lvl` source. Never panics on any input: every malformed
/// shape maps to [`LevelError`].
pub fn parse_str(path: &str, src: &str) -> Result<Level, LevelError> {
    // Split header / grid on the first `---` line.
    let lines: Vec<&str> = src.lines().collect();
    let sep = lines.iter().position(|l| l.trim() == "---");
    let Some(sep_idx) = sep else {
        return Err(LevelError::at(
            path,
            0,
            0,
            "missing `---` separator between TOML header and grid".to_string(),
        ));
    };
    let header_src = lines[..sep_idx].join("\n");
    // Separator-relative numbering: the `---` line is line 1.
    let grid_base: u32 = sep_idx as u32; // file line of `---` is sep_idx+1

    let header: toml::Table = header_src.parse().map_err(|e: toml::de::Error| {
        LevelError::at(path, 0, 0, format!("bad TOML header: {e}"))
    })?;

    // Grid shape first: unknown characters and ragged rows report before
    // header semantics, so a malformed grid names its line/column/char
    // even when the header has problems too (PLAN Phase 5 gate).

    // Grid rows: no blank lines, no comments, exactly GRID_W columns.
    let grid_lines = &lines[sep_idx + 1..];
    if grid_lines.is_empty() {
        return Err(LevelError::at(
            path,
            grid_base + 1,
            0,
            "no grid rows after `---`".to_string(),
        ));
    }
    if grid_lines.len() > GRID_ROWS_MAX {
        return Err(LevelError::at(
            path,
            grid_base + 1,
            0,
            format!(
                "{} grid rows exceed the max of {}",
                grid_lines.len(),
                GRID_ROWS_MAX
            ),
        ));
    }
    let mut bricks = Vec::new();
    for (ri, raw) in grid_lines.iter().enumerate() {
        // Separator-relative line: the `---` line is line 1, so the
        // first grid row is line 2 (PLAN Phase 5 gate).
        let line_no = ri as u32 + 2;
        // Strip one trailing CR (CRLF files); anything else trailing is an
        // explicit error per ADR-0007 (trailing whitespace is invisible).
        let row = raw.strip_suffix('\r').unwrap_or(raw);
        if row.is_empty() {
            return Err(LevelError::at(
                path,
                line_no,
                0,
                "blank line in grid (blank lines are allowed in the header only)".to_string(),
            ));
        }
        if row.ends_with(' ') || row.ends_with('\t') {
            return Err(LevelError::at(
                path,
                line_no,
                row.chars().count() as u32,
                "trailing whitespace in grid row (strip trailing spaces/tabs)".to_string(),
            ));
        }
        let cells: Vec<char> = row.chars().collect();
        // Unknown characters report before ragged widths, so a
        // malformed row names its line, column and character first.
        for (ci, ch) in cells.iter().enumerate() {
            match ch {
                '.' | '1'..='5' | 'S' | 'E' => {}
                _ => {
                    return Err(LevelError::at(
                        path,
                        line_no,
                        ci as u32 + 1,
                        format!("unknown character '{ch}'"),
                    ));
                }
            }
        }
        if cells.len() != GRID_W {
            return Err(LevelError::at(
                path,
                line_no,
                0,
                format!(
                    "grid row has {} columns, expected exactly {}",
                    cells.len(),
                    GRID_W
                ),
            ));
        }
        for (ci, ch) in cells.iter().enumerate() {
            let brick = match ch {
                '.' => None,
                '1'..='5' => Some(Brick::normal(ci as u8, ri as u8, *ch as u8 - b'0')),
                'S' => Some(Brick::steel(ci as u8, ri as u8)),
                // Validity was checked above; the wildcard is unreachable.
                _ => Some(Brick::explosive(ci as u8, ri as u8)),
            };
            if let Some(b) = brick {
                bricks.push(b);
            }
        }
    }
    // Header semantics after the grid shape: name/tier/knobs.
    let name = header
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LevelError::at(path, 0, 0, "header is missing `name`".to_string()))?
        .to_string();
    let tier = header
        .get("tier")
        .and_then(|v| v.as_integer())
        .ok_or_else(|| LevelError::at(path, 0, 0, "header is missing `tier`".to_string()))?;
    if !(i64::from(TIER_MIN)..=i64::from(TIER_MAX)).contains(&tier) {
        return Err(LevelError::at(
            path,
            0,
            0,
            format!("`tier` {tier} is not in {}..={}", TIER_MIN, TIER_MAX),
        ));
    }
    let ball_speed = match header.get("ball_speed") {
        None => 1.0,
        Some(v) => v
            .as_float()
            .filter(|x| x.is_finite() && *x > 0.0)
            .ok_or_else(|| {
                LevelError::at(
                    path,
                    0,
                    0,
                    "`ball_speed` must be a positive number".to_string(),
                )
            })? as f32,
    };
    let drop_rate = match header.get("drop_rate") {
        None => tuning::DROP_RATE_DEFAULT,
        Some(v) => v
            .as_float()
            .filter(|x| x.is_finite() && (0.0..=1.0).contains(x))
            .ok_or_else(|| {
                LevelError::at(
                    path,
                    0,
                    0,
                    "`drop_rate` must be a number in 0..=1".to_string(),
                )
            })? as f32,
    };
    let palette = header
        .get("palette")
        .and_then(|v| v.as_str())
        .unwrap_or("neon")
        .to_string();
    if !bricks.iter().any(|b| b.counts_for_clear()) {
        return Err(LevelError::at(
            path,
            grid_base + 1,
            0,
            "no destructible bricks (the level could never be cleared)".to_string(),
        ));
    }

    Ok(Level {
        name,
        tier: tier as u8,
        ball_speed,
        drop_rate,
        palette,
        bricks,
    })
}

/// Parse a `.lvl` file from disk. I/O failures map to file-level errors.
pub fn parse_file(path: &Path) -> Result<Level, LevelError> {
    let label = path.display().to_string();
    let src = std::fs::read_to_string(path)
        .map_err(|e| LevelError::at(&label, 0, 0, format!("cannot read file: {e}")))?;
    parse_str(&label, &src)
}

/// The compiled-in campaign: `(id, Result<Level, LevelError>)` per file,
/// sorted `NN-`. A baked-in parse failure is a build-time content bug;
/// `--validate` reports it without launching the game.
pub fn campaign() -> Vec<(String, Result<Level, LevelError>)> {
    CAMPAIGN
        .iter()
        .map(|(id, src)| (id.to_string(), parse_str(id, src)))
        .collect()
}

/// Phase 2 hard-coded level: a wall of 1-3 HP bricks with steel corners
/// and one explosive in the middle. Still the default when no `--level`
/// is given; the run structure (Phase 8) draws from the campaign instead.
pub fn default_bricks() -> Vec<Brick> {
    let mut bricks = Vec::new();
    for col in 2..16 {
        bricks.push(Brick::normal(col, 1, 1));
        bricks.push(Brick::normal(col, 2, 2));
    }
    for col in 4..14 {
        bricks.push(Brick::normal(col, 3, 3));
    }
    bricks.push(Brick::steel(0, 1));
    bricks.push(Brick::steel(17, 1));
    bricks.push(Brick::explosive(8, 3));
    bricks
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str =
        "name = \"scratch\"\ntier = 1\n---\n....11111.........\n....22222.........\n";

    #[test]
    fn valid_level_parses() {
        let lvl = parse_str("scratch", VALID).expect("valid level");
        assert_eq!(lvl.name, "scratch");
        assert_eq!(lvl.tier, 1);
        assert_eq!(lvl.ball_speed, 1.0);
        assert_eq!(lvl.drop_rate, tuning::DROP_RATE_DEFAULT);
        assert_eq!(lvl.bricks.len(), 10);
    }

    #[test]
    fn bad_char_names_line_column_char() {
        let src = "name = \"bad\"\n---\n....ZZZZ....\n";
        let err = parse_str("/tmp/bad.lvl", src).expect_err("must fail");
        assert_eq!(err.line, 2, "unexpected line: {err}");
        assert_eq!(err.column, 5, "unexpected column: {err}");
        assert!(err.message.contains('Z'), "unexpected message: {err}");
        assert_eq!(
            err.to_string(),
            "/tmp/bad.lvl: line 2, column 5: unknown character 'Z'"
        );
    }

    #[test]
    fn ragged_row_is_an_error_not_padding() {
        let src = "name = \"r\"\ntier = 1\n---\n....11111....\n";
        let err = parse_str("r", src).expect_err("ragged must fail");
        assert!(err.message.contains("13 columns"), "{err}");
    }

    #[test]
    fn trailing_whitespace_says_so() {
        let src = "name = \"r\"\ntier = 1\n---\n....11111........ \n";
        let err = parse_str("r", src).expect_err("trailing ws must fail");
        assert!(err.message.contains("trailing whitespace"), "{err}");
    }

    #[test]
    fn missing_tier_is_an_error() {
        let src = "name = \"r\"\n---\n....11111.........\n";
        let err = parse_str("r", src).expect_err("tier required");
        assert!(err.message.contains("tier"), "{err}");
    }

    #[test]
    fn tier_bounds_enforced() {
        for tier in [0, 5, -1] {
            let src = format!("name = \"r\"\ntier = {tier}\n---\n....11111.........\n");
            assert!(parse_str("r", &src).is_err(), "tier {tier}");
        }
    }

    #[test]
    fn steel_only_never_clears() {
        let src = "name = \"r\"\ntier = 1\n---\nSSSSSSSSSSSSSSSSSS\n";
        let err = parse_str("r", src).expect_err("steel-only");
        assert!(err.message.contains("destructible"), "{err}");
    }

    #[test]
    fn missing_separator_is_an_error() {
        let err = parse_str("r", "name = \"r\"\ntier = 1\n").expect_err("no sep");
        assert!(err.message.contains("---"), "{err}");
    }

    #[test]
    fn campaign_parses_clean() {
        let levels = campaign();
        assert!(levels.len() >= 16, "only {} levels", levels.len());
        for (id, res) in &levels {
            assert!(res.is_ok(), "{id}: {}", res.as_ref().err().unwrap());
        }
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// No input string, however malformed, panics (ADR-0007 rule 6).
        #[test]
        fn never_panics_on_any_input(s in "\\PC*") {
            let _ = parse_str("fuzz", &s);
        }

        /// Mutations of a valid level never panic either.
        #[test]
        fn never_panics_on_mutations(
            idx in 0usize..60,
            ch in proptest::char::any(),
        ) {
            let mut v: Vec<char> =
                "name = \"m\"\ntier = 2\n---\n....11111.........\n".chars().collect();
            if !v.is_empty() {
                let at = idx % v.len();
                v[at] = ch;
            }
            let s: String = v.into_iter().collect();
            let _ = parse_str("fuzz", &s);
        }
    }
}
