//! Hand-rolled 5x7 bitmap font (ADR-0005: no font crate).
//!
//! Owns glyph bitmaps (uppercase A–Z, digits 0–9, `: . / × % - + ! ?` per
//! MOCKUP §6) and text drawing. Drawn as crisp `S×S` blocks, so text scales
//! with the framebuffer without resampling. Double-size (10x14) is achieved
//! by pixel doubling for headings (MOCKUP §6).

use tiny_skia::{Color, Pixmap, Rect, Transform};

/// One glyph = 7 rows; each row is 5 bits (bit 0 = leftmost column).
type Glyph = [u8; 7];
const GLYPH_W: usize = 5;

const fn g(rows: [u8; 7]) -> Glyph {
    rows
}

/// Bit pattern helpers: `r` is a row of 5 `#`/`.` chars left→right.
const fn row(r: &str) -> u8 {
    let mut out = 0u8;
    let bytes = r.as_bytes();
    let mut i = 0;
    while i < 5 {
        out |= if bytes[i] == b'#' { 1 << (4 - i) } else { 0 };
        i += 1;
    }
    out
}

const fn glyph(r0: &str, r1: &str, r2: &str, r3: &str, r4: &str, r5: &str, r6: &str) -> Glyph {
    g([
        row(r0),
        row(r1),
        row(r2),
        row(r3),
        row(r4),
        row(r5),
        row(r6),
    ])
}

const A: Glyph = glyph(
    ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
);
const B: Glyph = glyph(
    "####.", "#...#", "#...#", "####.", "#...#", "#...#", "####.",
);
const C: Glyph = glyph(
    ".####", "#....", "#....", "#....", "#....", "#....", ".####",
);
const D: Glyph = glyph(
    "####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####.",
);
const E: Glyph = glyph(
    "#####", "#....", "#....", "####.", "#....", "#....", "#####",
);
const F: Glyph = glyph(
    "#####", "#....", "#....", "####.", "#....", "#....", "#....",
);
const G: Glyph = glyph(
    ".####", "#....", "#....", "#.###", "#...#", "#...#", ".####",
);
const H: Glyph = glyph(
    "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
);
const I: Glyph = glyph(
    "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####",
);
const J: Glyph = glyph(
    "..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##..",
);
const K: Glyph = glyph(
    "#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#",
);
const L: Glyph = glyph(
    "#....", "#....", "#....", "#....", "#....", "#....", "#####",
);
const M: Glyph = glyph(
    "#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#",
);
const N: Glyph = glyph(
    "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#",
);
const O: Glyph = glyph(
    ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
);
const P: Glyph = glyph(
    "####.", "#...#", "#...#", "####.", "#....", "#....", "#....",
);
const Q: Glyph = glyph(
    ".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#",
);
const R: Glyph = glyph(
    "####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#",
);
const S: Glyph = glyph(
    ".####", "#....", "#....", ".###.", "....#", "....#", "####.",
);
const T: Glyph = glyph(
    "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#..",
);
const U: Glyph = glyph(
    "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
);
const V: Glyph = glyph(
    "#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#..",
);
const W: Glyph = glyph(
    "#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#",
);
const X: Glyph = glyph(
    "#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#",
);
const Y: Glyph = glyph(
    "#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#..",
);
const Z: Glyph = glyph(
    "#####", "....#", "...#.", "..#..", ".#...", "#....", "#####",
);

const G0: Glyph = glyph(
    ".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###.",
);
const G1: Glyph = glyph(
    "..#..", ".##..", "..#..", "..#..", "..#..", "..#..", "#####",
);
const G2: Glyph = glyph(
    ".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####",
);
const G3: Glyph = glyph(
    "#####", "...#.", "..#..", "...#.", "....#", "#...#", ".###.",
);
const G4: Glyph = glyph(
    "...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#.",
);
const G5: Glyph = glyph(
    "#####", "#....", "####.", "....#", "....#", "#...#", ".###.",
);
const G6: Glyph = glyph(
    "..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###.",
);
const G7: Glyph = glyph(
    "#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#...",
);
const G8: Glyph = glyph(
    ".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###.",
);
const G9: Glyph = glyph(
    ".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##..",
);

const COLON: Glyph = glyph(
    ".....", "..#..", "..#..", ".....", "..#..", "..#..", ".....",
);
const DOT: Glyph = glyph(
    ".....", ".....", ".....", ".....", ".....", "..#..", "..#..",
);
const SLASH: Glyph = glyph(
    "....#", "...#.", "..#..", ".#...", "#....", ".....", ".....",
);
const TIMES: Glyph = glyph(
    ".....", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", ".....",
);
const PERCENT: Glyph = glyph(
    "##..#", "##..#", "...#.", "..#..", ".#...", "#..##", "#..##",
);
const MINUS: Glyph = glyph(
    ".....", ".....", ".....", "#####", ".....", ".....", ".....",
);
const PLUS: Glyph = glyph(
    ".....", "..#..", "..#..", "#####", "..#..", "..#..", ".....",
);
const EXCLAM: Glyph = glyph(
    "..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#..",
);
const QUESTION: Glyph = glyph(
    ".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#..",
);
const SPACE: Glyph = glyph(
    ".....", ".....", ".....", ".....", ".....", ".....", ".....",
);

/// Look up a glyph by ASCII char. Unknown chars render as space.
pub fn glyph_for(c: char) -> &'static Glyph {
    match c {
        'A'..='Z' => match c {
            'A' => &A,
            'B' => &B,
            'C' => &C,
            'D' => &D,
            'E' => &E,
            'F' => &F,
            'G' => &G,
            'H' => &H,
            'I' => &I,
            'J' => &J,
            'K' => &K,
            'L' => &L,
            'M' => &M,
            'N' => &N,
            'O' => &O,
            'P' => &P,
            'Q' => &Q,
            'R' => &R,
            'S' => &S,
            'T' => &T,
            'U' => &U,
            'V' => &V,
            'W' => &W,
            'X' => &X,
            'Y' => &Y,
            'Z' => &Z,
            _ => &SPACE,
        },
        '0'..='9' => match c {
            '0' => &G0,
            '1' => &G1,
            '2' => &G2,
            '3' => &G3,
            '4' => &G4,
            '5' => &G5,
            '6' => &G6,
            '7' => &G7,
            '8' => &G8,
            '9' => &G9,
            _ => &SPACE,
        },
        ':' => &COLON,
        '.' => &DOT,
        '/' => &SLASH,
        '×' => &TIMES,
        '%' => &PERCENT,
        '-' => &MINUS,
        '+' => &PLUS,
        '!' => &EXCLAM,
        '?' => &QUESTION,
        _ => &SPACE,
    }
}

/// Draw `text` starting at logical pixel (x, y), each glyph pixel rendered
/// as a `scale×scale` block (ADR-0003 integer scaling). 1px letter spacing,
/// 2px word spacing (MOCKUP §6).
pub fn draw_text(pixmap: &mut Pixmap, x: i32, y: i32, text: &str, color: Color, scale: u32) {
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(color);
    paint.anti_alias = false; // bitmap font: crisp blocks, no AA

    let mut cx = x;
    for ch in text.chars() {
        if ch == ' ' {
            cx += (GLYPH_W as i32 + 1) * scale as i32;
            continue;
        }
        // The font is uppercase; lowercase input renders as its uppercase
        // glyph (the HUD is uppercase anyway).
        let glyph = glyph_for(ch.to_ascii_uppercase());
        for (row_i, row_bits) in glyph.iter().enumerate() {
            for col in 0..GLYPH_W {
                if row_bits & (1 << (4 - col)) != 0 {
                    let rect = Rect::from_xywh(
                        (cx + col as i32 * scale as i32) as f32,
                        (y + row_i as i32 * scale as i32) as f32,
                        scale as f32,
                        scale as f32,
                    )
                    .expect("on-screen text rect");
                    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                }
            }
        }
        cx += (GLYPH_W as i32 + 1) * scale as i32;
    }
}

/// Measure the rendered width of `text` in logical pixels (for centering).
pub fn text_width(text: &str, scale: u32) -> i32 {
    let mut w = 0;
    for _ in text.chars() {
        w += (GLYPH_W as i32 + 1) * scale as i32;
    }
    if !text.is_empty() {
        w -= scale as i32; // no trailing letter-spacing after last glyph
    }
    w
}
