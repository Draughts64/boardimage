// 3x5 bitmap glyphs for digits and letters (notation rendering, SPEC §4.2).
// A bitmap font keeps the binary dependency-free (no embedded TTF yet —
// swap for ab_glyph later if richer typography is needed).

const FONT: &[(&str, u16)] = &[
    ("0", 0b111_101_101_101_111),
    ("1", 0b010_110_010_010_111),
    ("2", 0b111_001_111_100_111),
    ("3", 0b111_001_111_001_111),
    ("4", 0b101_101_111_001_001),
    ("5", 0b111_100_111_001_111),
    ("6", 0b111_100_111_101_111),
    ("7", 0b111_001_001_010_010),
    ("8", 0b111_101_111_101_111),
    ("9", 0b111_101_111_001_111),
    ("A", 0b111_101_111_101_101),
    ("B", 0b110_101_110_101_110),
    ("C", 0b111_100_100_100_111),
    ("D", 0b110_101_101_101_110),
    ("E", 0b111_100_110_100_111),
    ("F", 0b111_100_110_100_100),
    ("G", 0b111_100_101_101_111),
    ("H", 0b101_101_111_101_101),
    ("I", 0b111_010_010_010_111),
    ("J", 0b011_001_001_101_111),
    ("K", 0b101_110_100_110_101),
    ("L", 0b100_100_100_100_111),
    ("M", 0b101_111_111_101_101),
    ("N", 0b101_111_111_111_101),
    ("O", 0b111_101_101_101_111),
    ("P", 0b111_101_111_100_100),
    ("Q", 0b111_101_101_111_011),
    ("R", 0b111_101_110_110_101),
    ("S", 0b111_100_111_001_111),
    ("T", 0b111_010_010_010_010),
    ("U", 0b101_101_101_101_111),
    ("V", 0b101_101_101_101_010),
    ("W", 0b101_101_111_111_101),
    ("X", 0b101_101_010_101_101),
    ("Y", 0b101_101_010_010_010),
    ("Z", 0b111_001_010_100_111),
];

pub fn glyph(ch: char) -> Option<u16> {
    let s = ch.to_ascii_uppercase().to_string();
    FONT.iter().find(|(c, _)| *c == s).map(|(_, b)| *b)
}

/// Iterate the set pixels of a glyph: (col 0..3, row 0..5).
pub fn pixels(bits: u16) -> impl Iterator<Item = (u8, u8)> {
    let mut i = 0u8;
    std::iter::from_fn(move || {
        while i < 15 {
            let cur = i;
            i += 1;
            if bits & (1 << (14 - cur as u16)) != 0 {
                return Some((cur % 3, cur / 3));
            }
        }
        None
    })
}

pub fn text_width(s: &str) -> u32 {
    s.chars().count() as u32 * 4 + 1 // 3 cols + 1 spacing
}

/// Iterate set pixels for a whole string, x offset per char.
pub fn text_pixels(s: &str) -> impl Iterator<Item = (u8, u8)> {
    let chars: Vec<Option<u16>> = s.chars().map(glyph).collect();
    let mut idx = 0usize;
    let mut sub: Vec<(u8, u8)> = Vec::new();
    std::iter::from_fn(move || loop {
        if let Some(p) = sub.pop() {
            return Some(p);
        }
        if idx >= chars.len() {
            return None;
        }
        let off = (idx as u8) * 4;
        if let Some(bits) = chars[idx] {
            sub = pixels(bits).map(|(c, r)| (c + off, r)).collect();
        }
        idx += 1;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_and_letters() {
        assert!(glyph('0').is_some());
        assert!(glyph('k').is_some()); // lowercase maps to K
        assert!(glyph('!').is_none());
        let n: Vec<_> = text_pixels("10").collect();
        assert!(!n.is_empty());
    }
}
