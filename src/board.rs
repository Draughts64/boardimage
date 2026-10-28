// Board geometry: cell-number -> square coordinates (SPEC.md §4.1).
//
// Numbering follows the lidraughts engine / FMJD canon: cell 1 is the
// first playing square of the TOP row, counted left-to-right, rows
// top-to-bottom (white pieces 31-50 end up at the bottom). This equals
// PDN GameType notation digit 2 for white-starting games.
//
// `numbering` (absolute origin of cell 1): 0 bottom-left, 1 bottom-right,
// 2 top-left (engine/FMJD default), 3 top-right. When reading a PDN
// GameType tag, the digit is "from the starting player's perspective"
// and is converted here (black-starting games rotate the board 180°:
// absolute = 3 - digit).
//
// `light_squares`: playing squares are LIGHT (italian/spanish) —
// inverts the checkerboard phase AND the mapping of numbers.
// `all_squares`: every cell is numbered and playable (turkish).

#[derive(Debug, Clone, Copy)]
pub struct BoardDef {
    pub width: u16,
    pub height: u16,
    pub light_squares: bool,
    pub numbering: u8, // 0..3
    pub all_squares: bool,
}

impl BoardDef {
    pub fn cells(&self) -> u16 {
        self.width * self.height
    }
    pub fn playing_cells(&self) -> u16 {
        self.cells() / 2
    }
    pub fn max_square(&self) -> u16 {
        if self.all_squares {
            self.cells()
        } else {
            self.playing_cells()
        }
    }

    /// Is (x, y) a playing square? Coordinates from bottom-left, 0-based.
    pub fn is_playing(&self, x: u16, y: u16) -> bool {
        if self.all_squares {
            return true;
        }
        let dark = (x + y).is_multiple_of(2);
        dark != self.light_squares
    }

    fn phase(&self, row: u16) -> u16 {
        (row % 2)
            ^ u16::from(self.light_squares)
            ^ u16::from(self.numbering >= 2)
            ^ u16::from(self.numbering % 2 == 1)
    }

    fn xy_raw(&self, square: u16) -> Option<(u16, u16)> {
        if square == 0 || square > self.max_square() {
            return None;
        }
        let per_row = if self.all_squares {
            self.width
        } else {
            self.width / 2
        };
        let i = square - 1;
        let (row, pos) = (i / per_row, i % per_row);
        let mut x = if self.all_squares {
            pos
        } else {
            pos * 2 + self.phase(row)
        };
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

    /// Map a 1-based cell number to (x, y) from bottom-left.
    pub fn xy(&self, square: u16) -> Option<(u16, u16)> {
        self.xy_raw(square)
    }

    /// Cell number for (x, y), if it is a playing square.
    pub fn square_at(&self, x: u16, y: u16) -> Option<u16> {
        self.xy_iter()
            .find(|&(_n, ax, ay)| ax == x && ay == y)
            .map(|(n, _, _)| n)
    }

    pub fn xy_iter(&self) -> impl Iterator<Item = (u16, u16, u16)> + '_ {
        let max = self.max_square();
        (1..=max).map(move |n| {
            let (x, y) = self.xy_raw(n).expect("xy_raw in range");
            (n, x, y)
        })
    }

    /// File letter for column x. On 10-wide boards the draughts canon
    /// skips 'j' (files a-i, k); 'j' is accepted on input as an alias
    /// for the 10th file (chess-style tools).
    pub fn file_letter(&self, x: u16) -> char {
        if self.width == 10 && x == 9 {
            'k'
        } else {
            (b'a' + (x as u8 % 26)) as char
        }
    }

    /// Algebraic name of a cell, e.g. "a1" (rank 1 = bottom row).
    pub fn algebraic(&self, x: u16, y: u16) -> String {
        format!("{}{}", self.file_letter(x), y + 1)
    }

    /// Cell number for an algebraic name ("a1".."k10"); the cell must be
    /// a playing square of this board.
    pub fn alg_square(&self, name: &str) -> Option<u16> {
        let mut cs = name.chars();
        let c = cs.next()?.to_ascii_lowercase();
        if !c.is_ascii_lowercase() {
            return None;
        }
        let digits: String = cs.collect();
        if digits.is_empty() || !digits.bytes().all(|d| d.is_ascii_digit()) {
            return None;
        }
        let rank: u16 = digits.parse().ok()?;
        if rank == 0 || rank > self.height {
            return None;
        }
        let x = match c {
            // 10-wide: canon skips 'j'; accept chess-style 'j' as file 10
            'j' if self.width == 10 => 9,
            'k' if self.width == 10 => 9,
            c if c.is_ascii_lowercase() => (c as u8 - b'a') as u16,
            _ => return None,
        };
        if x >= self.width {
            return None;
        }
        let y = rank - 1;
        self.square_at(x, y)
    }
}

/// One square identifier: numeric ("31", "31-50" ranges handled by the
/// caller) or algebraic ("a1"). Shared by fen, lastMove, squares, arrows.
pub fn parse_square(tok: &str, b: &BoardDef) -> Option<u16> {
    let tok = tok.trim();
    if tok.is_empty() {
        return None;
    }
    if tok.bytes().next().is_some_and(|d| d.is_ascii_digit()) {
        let n: u16 = tok.parse().ok()?;
        (1..=b.max_square()).contains(&n).then_some(n)
    } else {
        b.alg_square(tok)
    }
}

/// Parse a from-to move: "32-28", "3228" (engine UCI concatenation),
/// "c3-b4" or "c3b4" (algebraic).
pub fn parse_move_squares(s: &str, b: &BoardDef) -> Option<(u16, u16)> {
    let s = s.trim();
    if let Some((a, c)) = s.split_once('-') {
        return Some((parse_square(a, b)?, parse_square(c, b)?));
    }
    if s.is_empty() {
        return None;
    }
    if s.bytes().all(|d| d.is_ascii_digit()) {
        // numeric concatenation; split in half (equal-length halves)
        if !s.len().is_multiple_of(2) || !(4..=8).contains(&s.len()) {
            return None;
        }
        let h = s.len() / 2;
        let a: u16 = s[..h].parse().ok()?;
        let c: u16 = s[h..].parse().ok()?;
        return (a >= 1 && c >= 1 && a <= b.max_square() && c <= b.max_square()).then_some((a, c));
    }
    // algebraic concatenation "c3b4": [letter digits] x2
    let bytes = s.as_bytes();
    let mut i = 0;
    if !bytes.first().is_some_and(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let mid = i;
    if mid >= bytes.len() || !bytes[mid].is_ascii_alphabetic() {
        return None;
    }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i != bytes.len() {
        return None;
    }
    Some((b.alg_square(&s[..mid])?, b.alg_square(&s[mid..])?))
}

/// PDN gameType number -> BoardDef + variant name (SPEC.md §2.4).
/// Numbering is the lidraughts-engine canon: cell 1 top-left (=2) for
/// the international family; czech/turkish/spantsiretti number from
/// the bottom-left (=0) per the PDN table.
pub fn from_game_type(gt: u32) -> Option<(BoardDef, &'static str)> {
    let (w, h, light, num, all, name) = match gt {
        20 => (10, 10, false, 2, false, "standard"),
        21 => (8, 8, false, 2, false, "english"),
        22 => (8, 8, true, 2, false, "italian"),
        23 => (8, 8, false, 2, false, "pool"),
        24 => (8, 8, true, 1, false, "spanish"),
        25 => (8, 8, false, 2, false, "russian"),
        26 => (8, 8, false, 2, false, "brazilian"),
        27 => (12, 12, false, 2, false, "canadian"),
        28 => (8, 8, true, 1, false, "portuguese"),
        29 => (8, 8, false, 0, false, "czech"),
        30 => (8, 8, false, 0, true, "turkish"),
        31 => (8, 8, false, 1, false, "thai"),
        40 => (10, 10, false, 2, false, "frisian"),
        41 => (10, 8, false, 0, false, "spanciretti"),
        96 => (10, 10, false, 2, false, "breakthrough"),
        97 => (10, 10, false, 2, false, "frysk"),
        98 => (10, 10, false, 2, false, "antidraughts"),
        _ => return None,
    };
    Some((
        BoardDef {
            width: w,
            height: h,
            light_squares: light,
            numbering: num,
            all_squares: all,
        },
        name,
    ))
}

/// Full PDN GameType tag: "21,B,8,8,N1,0" or just "21".
/// Returns the board, variant name, and the tag's notation letter
/// (A/N/S) when the full form was given.
pub fn from_game_type_str(s: &str) -> Option<(BoardDef, &'static str, Option<char>)> {
    let mut it = s.trim().split(',');
    let gt: u32 = it.next()?.trim().parse().ok()?;
    let (mut def, name) = from_game_type(gt)?;
    let rest: Vec<&str> = it.map(str::trim).collect();
    let mut notation = None;
    if rest.len() >= 4 {
        let start = rest[0].trim().to_ascii_uppercase();
        let w: u16 = rest[1].parse().ok()?;
        let h: u16 = rest[2].parse().ok()?;
        let nn = rest[3].trim();
        let mut cs = nn.chars();
        let letter = cs.next()?.to_ascii_uppercase();
        let digit = cs.next()?.to_digit(10)? as u8;
        if !matches!(letter, 'A' | 'N' | 'S') || digit > 3 {
            return None;
        }
        if !matches!(start.as_str(), "W" | "B") {
            return None;
        }
        if !((2..=16).contains(&w)
            && w.is_multiple_of(2)
            && (2..=16).contains(&h)
            && h.is_multiple_of(2))
        {
            return None;
        }
        let invert = rest.get(4).and_then(|v| v.trim().parse::<u8>().ok());
        def.width = w;
        def.height = h;
        def.light_squares = invert == Some(1);
        // tag digit is from the STARTING player's perspective
        def.numbering = if start == "B" { 3 - digit } else { digit };
        notation = Some(letter);
    } else if !rest.is_empty() {
        return None; // malformed tag
    }
    Some((def, name, notation))
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
    fn international_engine_canon() {
        let b = from_game_type(20).unwrap().0;
        // engine canon: cell 1 = top-left playing square, cell 50 bottom-right
        assert_eq!(b.xy(1), Some((1, 9)));
        assert_eq!(b.xy(5), Some((9, 9)));
        assert_eq!(b.xy(46), Some((0, 0)));
        assert_eq!(b.xy(50), Some((8, 0)));
        assert_eq!(b.max_square(), 50);
    }

    #[test]
    fn international_white_bottom() {
        let b = from_game_type(20).unwrap().0;
        // white 31-50 occupies the bottom four rows
        assert!(b
            .xy_iter()
            .filter(|&(n, _x, _y)| (31..=50).contains(&n))
            .all(|(_n, _x, y)| y <= 3));
    }

    #[test]
    fn russian_white_bottom() {
        let b = from_game_type(25).unwrap().0;
        assert!(b
            .xy_iter()
            .filter(|&(n, _x, _y)| (21..=32).contains(&n))
            .all(|(_n, _x, y)| y <= 2));
    }

    #[test]
    fn italian_flips_phase_and_origin() {
        let b = from_game_type(22).unwrap().0;
        assert!(b.light_squares);
        // numbering=2: cell 1 is top-left LIGHT square
        let (x, y) = b.xy(1).unwrap();
        assert_eq!((x, y), (0, 7));
        assert!(b.is_playing(x, y));
    }

    #[test]
    fn spanish_bottom_right_origin() {
        let b = from_game_type(24).unwrap().0;
        assert_eq!(b.xy(1), Some((7, 0)));
    }

    #[test]
    fn turkish_all_squares() {
        let b = from_game_type(30).unwrap().0;
        assert!(b.all_squares);
        assert_eq!(b.max_square(), 64);
        assert_eq!(b.xy(1), Some((0, 0)));
        assert_eq!(b.xy(64), Some((7, 7)));
        assert_eq!(b.alg_square("a1"), Some(1));
    }

    #[test]
    fn canadian_is_144() {
        let b = from_game_type(27).unwrap().0;
        assert_eq!(b.max_square(), 72);
        assert_eq!(b.width, 12);
    }

    #[test]
    fn algebraic_letters() {
        let b = from_game_type(20).unwrap().0;
        assert_eq!(b.file_letter(0), 'a');
        assert_eq!(b.file_letter(8), 'i');
        assert_eq!(b.file_letter(9), 'k'); // j skipped on 10-wide
                                           // FMJD corners: a1=46, k10=5
        assert_eq!(b.alg_square("a1"), Some(46));
        assert_eq!(b.alg_square("k10"), Some(5));
        assert_eq!(b.alg_square("j10"), Some(5)); // chess-style alias
        assert_eq!(b.alg_square("a10"), None); // a10 is not a playing square
        assert_eq!(b.algebraic(0, 0), "a1");
        assert_eq!(b.algebraic(9, 9), "k10");
    }

    #[test]
    fn english_algebraic_matches_engine() {
        // engine Pos64: fields 29-32 = algebraic rank 1 (a1,c1,e1,g1)
        let b = from_game_type(21).unwrap().0;
        assert_eq!(b.alg_square("a1"), Some(29));
        assert_eq!(b.alg_square("g1"), Some(32));
        assert_eq!(b.alg_square("b8"), Some(1));
        assert_eq!(b.alg_square("c3"), Some(22));
    }

    #[test]
    fn game_type_tag_strings() {
        let (b, name, notation) = from_game_type_str("21,B,8,8,N1,0").unwrap();
        assert_eq!(name, "english");
        assert_eq!(notation, Some('N'));
        assert_eq!(b.numbering, 2); // 3 - 1 (black starts)
        let (b, _, _) = from_game_type_str("22,W,8,8,N2,1").unwrap();
        assert!(b.light_squares);
        assert_eq!(b.numbering, 2);
        let (b, _, notation) = from_game_type_str("41,W,10,8,A0,0").unwrap();
        assert_eq!((b.width, b.height), (10, 8));
        assert_eq!(notation, Some('A'));
        let (_, name, notation) = from_game_type_str("25").unwrap();
        assert_eq!(name, "russian");
        assert_eq!(notation, None);
        assert!(from_game_type_str("25,W").is_none());
        assert!(from_game_type_str("xx").is_none());
    }

    #[test]
    fn move_squares() {
        let b = from_game_type(20).unwrap().0;
        assert_eq!(parse_move_squares("32-28", &b), Some((32, 28)));
        assert_eq!(parse_move_squares("3228", &b), Some((32, 28)));
        assert_eq!(parse_move_squares("46-41", &b), Some((46, 41)));
        assert_eq!(parse_move_squares("a1-f6", &b), Some((46, 23)));
        assert_eq!(parse_move_squares("a1f6", &b), Some((46, 23)));
        assert_eq!(parse_move_squares("99-99", &b), None);
        assert_eq!(parse_move_squares("32281", &b), None);
        let e = from_game_type(21).unwrap().0;
        assert_eq!(parse_move_squares("c3-b4", &e), Some((22, 17)));
    }
}
