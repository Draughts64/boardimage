// Draughts FEN: "<turn>:<white>:<black>" (SPEC.md §3).
// Takes the first 3 ':'-separated parts; tail (move counters) ignored.
// Pieces: plain number = man, K = king, G/P = ghost man/king (opacity).
// Lists: comma-separated; ranges via '-' ("W31-50", "K46-50").

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Man,
    King,
    GhostMan,
    GhostKing,
}

impl Kind {
    pub fn is_king(self) -> bool {
        matches!(self, Kind::King | Kind::GhostKing)
    }
    pub fn is_ghost(self) -> bool {
        matches!(self, Kind::GhostMan | Kind::GhostKing)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Piece {
    pub square: u16, // 1-based cell number
    pub kind: Kind,
}

#[derive(Debug, Clone, Default)]
pub struct Position {
    pub white: Vec<Piece>,
    pub black: Vec<Piece>,
}

impl Position {
    /// All pieces, white first (stable order for rendering).
    pub fn pieces(&self) -> impl Iterator<Item = (bool, Piece)> + '_ {
        self.white
            .iter()
            .map(|&p| (true, p))
            .chain(self.black.iter().map(|&p| (false, p)))
    }
}

pub fn parse(fen: &str, cells: u16) -> Result<Position, &'static str> {
    let mut parts = fen.split(':');
    let _turn = parts
        .next()
        .filter(|t| *t == "W" || *t == "B")
        .ok_or("invalid fen")?;
    let mut pos = Position::default();
    for (is_white, part) in [(true, parts.next()), (false, parts.next())] {
        let part = part.ok_or("invalid fen")?;
        let list = if is_white {
            &mut pos.white
        } else {
            &mut pos.black
        };
        // fields carry a side marker: "W31-50" / "B1,2"
        let part = part
            .strip_prefix('W')
            .or_else(|| part.strip_prefix('B'))
            .unwrap_or(part);
        for item in part.split(',') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            let (kind, range) = if let Some(r) = item.strip_prefix('K') {
                (Kind::King, r)
            } else if let Some(r) = item.strip_prefix('G') {
                (Kind::GhostMan, r)
            } else if let Some(r) = item.strip_prefix('P') {
                (Kind::GhostKing, r)
            } else {
                (Kind::Man, item)
            };
            let (lo, hi) = match range.split_once('-') {
                Some((a, b)) => (
                    a.parse::<u16>().map_err(|_| "invalid fen")?,
                    b.parse::<u16>().map_err(|_| "invalid fen")?,
                ),
                None => {
                    let n = range.parse::<u16>().map_err(|_| "invalid fen")?;
                    (n, n)
                }
            };
            if lo == 0 || hi < lo || hi > cells {
                return Err("invalid fen");
            }
            for sq in lo..=hi {
                list.push(Piece { square: sq, kind });
            }
        }
    }
    Ok(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn international_start() {
        let p = parse("W:W31-50:B1-20", 50).unwrap();
        assert_eq!(p.white.len(), 20);
        assert_eq!(p.black.len(), 20);
        assert_eq!(p.white[0].square, 31);
    }

    #[test]
    fn kings_and_ghosts() {
        let p = parse("W:W46,K47,G48,P49:B1,2", 50).unwrap();
        assert!(p.white[1].kind.is_king());
        assert!(p.white[2].kind.is_ghost());
        assert!(p.white[3].kind.is_king() && p.white[3].kind.is_ghost());
        assert_eq!(p.black.len(), 2);
    }

    #[test]
    fn russian_start() {
        let p = parse("W:W21-32:B1-12:H0:F1", 32).unwrap();
        assert_eq!(p.white.len(), 12);
    }

    #[test]
    fn errors() {
        assert!(parse("", 50).is_err());
        assert!(parse("X:W1:B2", 50).is_err());
        assert!(parse("W:W51:B1", 50).is_err());
        assert!(parse("W:Wx:B1", 50).is_err());
        assert!(parse("W:W10-1:B1", 50).is_err());
    }
}
