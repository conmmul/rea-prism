//! Approximate text metrics.
//!
//! Laying out margins needs a width for every tick label before any backend is
//! chosen, so we estimate from average glyph advances rather than loading a font.
//! The GUI refines placement with its own metrics when it paints.

/// Average advance per character as a fraction of the font size, for the
/// sans-serif faces Prism defaults to.
const AVG_ADVANCE: f32 = 0.52;
const DIGIT_ADVANCE: f32 = 0.56;

/// Estimated width in points of `text` rendered at `size`.
pub fn width(text: &str, size: f32) -> f32 {
    let mut w = 0.0;
    for c in text.chars() {
        w += size
            * match c {
                '0'..='9' => DIGIT_ADVANCE,
                'i' | 'l' | 'j' | '.' | ',' | '\'' | '|' | ':' | ';' => 0.26,
                'm' | 'w' | 'M' | 'W' => 0.85,
                ' ' => 0.28,
                'A'..='Z' => 0.68,
                _ => AVG_ADVANCE,
            };
    }
    w
}

/// Height of one line of text at `size`.
pub fn line_height(size: f32) -> f32 {
    size * 1.2
}

/// Width of the widest line in a multi-line string.
pub fn block_width(text: &str, size: f32) -> f32 {
    text.lines().map(|l| width(l, size)).fold(0.0, f32::max)
}

/// Height of a multi-line string.
pub fn block_height(text: &str, size: f32) -> f32 {
    line_height(size) * text.lines().count().max(1) as f32
}
