// Board geometry: cell-number -> square coordinates (SPEC.md §4.1).
//
// PDN numbering (default 0): cell 1 is the first playing square of the
// bottom row, counted left-to-right, rows bottom-to-top, over PLAYING
// squares only (half the squares on diagonal variants).
//
// `numbering` (PDN 0..3): where cell 1 starts — 0 bottom-left, 1
// bottom-right, 2 top-left (italian!), 3 top-right.
// `light_squares`: playing squares are LIGHT (italian/spanish) —
// inverts the checkerboard phase AND the mapping of numbers.

#[derive(Debug, Clone, Copy)]
pub struct BoardDef {
    pub width: u16,
    pub height: u16,
    pub light_squares: bool,
    pub numbering: u8, // 0..3
}

impl BoardDef {
    pub fn cells(&self) -> u16 {
        self.width * self.height
    }
    pub fn playing_cells(&self) -> u16 {
        self.cells() / 2
    }
    pub fn max_square(&self) -> u16 {
        if self.width.is_multiple_of(2) && self.height.is_multiple_of(2) {
            self.playing_cells()
        } else {
            self.cells() // all-squares variants (odd boards not in v1)
        }
    }

    /// Is (x, y) a playing square? Coordinates from bottom-left, 0-based.
    pub fn is_playing(&self, x: u16, y: u16) -> bool {
        let dark = (x + y).is_multiple_of(2);
        dark != self.light_squares
    }

    /// Map a 1-based cell number to (x, y) from bottom-left.
    pub fn xy(&self, square: u16) -> Option<(u16, u16)> {
        if square == 0 || square > self.max_square() {
            return None;
        }
        let per_row = self.width / 2;
        let i = square - 1;
        let (row, pos) = (i / per_row, i % per_row);
        // Rows alternate the phase offset; row 0 carries the base phase.
        let phase = (row % 2)
            ^ u16::from(self.light_squares)
            ^ u16::from(self.numbering >= 2)
            ^ u16::from(self.numbering % 2 == 1);
        let mut x = pos * 2 + phase;
        let mut y = row;
        // numbering 1/3: mirror columns (first cell bottom/top-RIGHT)
        if self.numbering % 2 == 1 {
            x = self.width - 1 - x;
        }
        // numbering 2/3: rows counted top-to-bottom
        if self.numbering >= 2 {
            y = self.height - 1 - y;
        }
        Some((x, y))
    }

    /// Cell number for (x, y), if it is a playing square.
    pub fn square_at(&self, x: u16, y: u16) -> Option<u16> {
        self.xy_iter()
            .find(|&(_n, ax, ay)| ax == x && ay == y)
            .map(|(n, _, _)| n)
    }

    pub fn xy_iter(&self) -> impl Iterator<Item = (u16, u16, u16)> + '_ {
        let per_row = self.width / 2;
        let max = self.max_square();
        (0..max).map(move |i| {
            let (row, pos) = (i / per_row, i % per_row);
            let phase = (row % 2)
                ^ u16::from(self.light_squares)
                ^ u16::from(self.numbering >= 2)
                ^ u16::from(self.numbering % 2 == 1);
            let mut x = pos * 2 + phase;
            let mut y = row;
            if self.numbering % 2 == 1 {
                x = self.width - 1 - x;
            }
            if self.numbering >= 2 {
                y = self.height - 1 - y;
            }
            (i + 1, x, y)
        })
    }
}

/// PDN gameType -> BoardDef + variant name (SPEC.md §2.4).
pub fn from_game_type(gt: u32) -> Option<(BoardDef, &'static str)> {
    let (w, h, light, num, name) = match gt {
        20 => (10, 10, false, 0, "standard"),
        21 => (8, 8, false, 0, "english"),
        22 => (8, 8, true, 2, "italian"),
        23 => (8, 8, false, 0, "pool"),
        24 => (8, 8, true, 1, "spanish"),
        25 => (8, 8, false, 0, "russian"),
        26 => (8, 8, false, 0, "brazilian"),
        27 => (12, 12, false, 0, "canadian"),
        28 => (8, 8, true, 1, "portuguese"),
        29 => (8, 8, false, 0, "czech"),
        30 => (8, 8, false, 0, "turkish"),
        31 => (8, 8, false, 0, "thai"),
        40 => (10, 10, false, 0, "frisian"),
        41 => (10, 8, false, 0, "spanciretti"),
        96 => (10, 10, false, 0, "breakthrough"),
        97 => (10, 10, false, 0, "frysk"),
        98 => (10, 10, false, 0, "antidraughts"),
        _ => return None,
    };
    Some((
        BoardDef {
            width: w,
            height: h,
            light_squares: light,
            numbering: num,
        },
        name,
    ))
}

pub fn from_variant(name: &str) -> Option<(BoardDef, &'static str)> {
    let names = [
        (20u32, "standard"),
        (21, "english"),
        (22, "italian"),
        (23, "pool"),
        (24, "spanish"),
        (25, "russian"),
        (26, "brazilian"),
        (27, "canadian"),
        (28, "portuguese"),
        (29, "czech"),
        (30, "turkish"),
        (31, "thai"),
        (40, "frisian"),
        (41, "spanciretti"),
        (96, "breakthrough"),
        (97, "frysk"),
        (98, "antidraughts"),
    ];
    names
        .iter()
        .find(|(_, n)| *n == name)
        .and_then(|(gt, _)| from_game_type(*gt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn international_mapping() {
        let b = from_game_type(20).unwrap().0;
        // Cell 1: bottom-left playing (dark) square
        assert_eq!(b.xy(1), Some((0, 0)));
        // Cell 6: second row starts on the shifted phase
        assert_eq!(b.xy(6), Some((1, 1)));
        assert_eq!(b.xy(50), Some((9, 9)));
        assert_eq!(b.max_square(), 50);
    }

    #[test]
    fn italian_flips_phase_and_origin() {
        let b = from_game_type(22).unwrap().0;
        assert!(b.light_squares);
        // numbering=2: cell 1 is top-left LIGHT square
        let (x, y) = b.xy(1).unwrap();
        assert_eq!((x, y), (0, 7));
        assert!(b.is_playing(x, y));
        // same physical square holds a different number than in russian:
        let r = from_game_type(25).unwrap().0;
        assert_eq!(r.xy(1), Some((0, 0))); // russian cell 1 bottom-left
    }

    #[test]
    fn canadian_is_144() {
        let b = from_game_type(27).unwrap().0;
        assert_eq!(b.max_square(), 72);
        assert_eq!(b.width, 12);
    }

    #[test]
    fn spanish_bottom_right_origin() {
        let b = from_game_type(24).unwrap().0;
        assert_eq!(b.xy(1), Some((7, 0)));
    }
}
