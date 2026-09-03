//! `.lvl` parsing and the brick grid (ADR-0007, Phase 5).
//!
//! Owns the TOML-header + ASCII-grid parser with path/line/column/char
//! errors (FR-36) and the in-memory brick grid. Never panics on any input
//! (property-tested).
//!
//! Phase 2: only the hard-coded level for the core loop. The full parser
//! arrives in Phase 5.

use super::physics::Brick;

/// Phase 2 hard-coded level: a wall of 1-3 HP bricks with steel corners
/// and one explosive in the middle. Clearable, losable, boring on purpose.
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
