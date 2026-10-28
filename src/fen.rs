// Draughts FEN per PDN 3.0 (wiegerw.github.io/pdn/fen.html), extended
// to accept what real generators emit:
//   "<turn>:<W|B><piece list>[:<W|B><piece list>][:H0:F1...]"
// - turn: W, B or ? (unknown), case-insensitive
// - side lists: colour marker + comma-separated squares; a side may be
//   empty ("B") and unknown trailing fields (lidraughts :H0:F1) are skipped
// - squares: numeric ("31", ranges "31-50") OR algebraic ("a1", "Kc3"),
//   mixed freely; prefixes K = king, G/P = ghost man/king (lidraughts)
// Algebraic names are physical (rank 1 = bottom) and independent of the
// numbering mode; see board.rs for the per-board file letters.

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

pub fn parse(fen: &str, b: &crate::board::BoardDef) -> Result<Position, &'static str> {
    let fen: String = fen.chars().filter(|c| !c.is_whitespace()).collect();
    let mut parts = fen.split(':');
    match parts.next().unwrap_or("").to_ascii_uppercase().as_str() {
        "W" | "B" | "?" => {}
        _ => return Err("invalid fen"),
    }
    let mut pos = Position::default();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        let (is_white, toks) = match part.chars().next().unwrap().to_ascii_uppercase() {
            'W' => (true, &part[1..]),
            'B' => (false, &part[1..]),
            // lidraughts extension fields (:H0, :F1) and unknown tags: skip
            _ => continue,
        };
        let list = if is_white {
            &mut pos.white
        } else {
            &mut pos.black
        };
        for item in toks.split(',') {
            if item.is_empty() {
                continue; // empty side list ("B") or trailing comma
            }
            let (kind, tok) = match item.chars().next().unwrap().to_ascii_uppercase() {
                'K' => (Kind::King, &item[1..]),
                'G' => (Kind::GhostMan, &item[1..]),
                'P' => (Kind::GhostKing, &item[1..]),
                _ => (Kind::Man, item),
            };
            if tok.is_empty() {
                return Err("invalid fen");
            }
            if tok.chars().next().unwrap().is_ascii_digit() {
                let (lo, hi) = match tok.split_once('-') {
                    Some((a, c)) => (
                        a.parse::<u16>().map_err(|_| "invalid fen")?,
                        c.parse::<u16>().map_err(|_| "invalid fen")?,
                    ),
                    None => {
                        let n = tok.parse::<u16>().map_err(|_| "invalid fen")?;
                        (n, n)
                    }
                };
                if lo == 0 || hi < lo || hi > b.max_square() {
                    return Err("invalid fen");
                }
                for sq in lo..=hi {
                    list.push(Piece { square: sq, kind });
                }
            } else {
                // algebraic square; no ranges per the spec grammar
                let sq = b.alg_square(tok).ok_or("invalid fen")?;
                list.push(Piece { square: sq, kind });
            }
        }
    }
    Ok(pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{from_game_type, from_game_type_str};

    fn std10() -> crate::board::BoardDef {
        from_game_type(20).unwrap().0
    }
    fn eng8() -> crate::board::BoardDef {
        from_game_type(21).unwrap().0
    }

    #[test]
    fn international_start() {
        let p = parse("W:W31-50:B1-20", &std10()).unwrap();
        assert_eq!(p.white.len(), 20);
        assert_eq!(p.black.len(), 20);
        assert_eq!(p.white[0].square, 31);
    }

    #[test]
    fn engine_initial_with_counters() {
        // exact Forsyth.initial of the lidraughts engine
        let p = parse(
            "W:W31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50:B1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20:H0:F1",
            &std10(),
        )
        .unwrap();
        assert_eq!(p.white.len(), 20);
        assert_eq!(p.black.len(), 20);
    }

    #[test]
    fn pdn_spec_examples() {
        // examples from wiegerw.github.io/pdn/fen.html (8x8 boards)
        for fen in [
            "B:W18,24,27,28,K10,K15:B12,16,20,K22,K25,K29",
            "B:W18,19,21,23,24,26,29,30,31,32:B1,2,3,4,6,7,9,10,11,12",
        ] {
            assert!(parse(fen, &eng8()).is_ok());
        }
        let p = parse("B:W18,24,27,28,K10,K15:B12,16,20,K22,K25,K29", &eng8()).unwrap();
        assert_eq!(p.white.len(), 6);
        assert_eq!(p.black.len(), 6);
        assert!(p.white[4].kind.is_king()); // K10
        assert!(p.black[3].kind.is_king()); // K22
    }

    #[test]
    fn algebraic_user_fen() {
        // the FEN that started it all: english 8x8, algebraic squares
        let p = parse(
            "W:Wa3,g3,b2,d2,f2,h2,a1,c1,e1,g1:Bb8,d8,f8,h8,a7,c7,e7,g7,b6,d6,f6,h6,Kc3,Ke3",
            &eng8(),
        )
        .unwrap();
        assert_eq!(p.white.len(), 10);
        assert_eq!(p.black.len(), 14);
        // engine-compatible numbers: a1=29, b8=1, c3=22
        assert!(p.white.iter().any(|&Piece { square, .. }| square == 29));
        assert!(p.black.iter().any(|&Piece { square, .. }| square == 1));
        let kc3 = p
            .black
            .iter()
            .find(|&&Piece { square, .. }| square == 22)
            .unwrap();
        assert!(kc3.kind.is_king());
    }

    #[test]
    fn mixed_notations_and_tolerances() {
        let b = std10();
        // mixed numeric + algebraic in one list
        assert!(parse("W:W46,a1:B1", &b).is_ok());
        // unknown turn colour
        assert!(parse("?:Wa1:Bb8", &eng8()).is_ok());
        // lowercase everything
        assert!(parse("w:wa1,g1:bb8,kb8", &eng8()).is_ok());
        // empty side list
        let p = parse("W:W31,32:B", &b).unwrap();
        assert_eq!(p.black.len(), 0);
        // ghost prefixes
        let p = parse("W:W46,K47,G48,P49:B1,2", &b).unwrap();
        assert!(p.white[1].kind.is_king());
        assert!(p.white[2].kind.is_ghost());
        assert!(p.white[3].kind.is_king() && p.white[3].kind.is_ghost());
        // all-squares (turkish) accepts numbers above the diagonal count
        let t = from_game_type(30).unwrap().0;
        assert!(parse("W:W33,64:B1", &t).is_ok());
    }

    #[test]
    fn game_type_tag_board() {
        let (b, _, _) = from_game_type_str("22,W,8,8,N2,1").unwrap();
        // algebraic names on the italian light-square board
        let p = parse("W:Wb1,c2:Bf7", &b).unwrap();
        assert_eq!(p.white.len(), 2);
        assert_eq!(p.black.len(), 1);
    }

    #[test]
    fn errors() {
        let b = std10();
        let e = eng8();
        assert!(parse("", &b).is_err());
        assert!(parse("X:W1:B2", &b).is_err());
        assert!(parse("W:W51:B1", &b).is_err());
        assert!(parse("W:Wx:B1", &b).is_err());
        assert!(parse("W:W10-1:B1", &b).is_err());
        assert!(parse("W:Wa11:B1", &e).is_err()); // rank above board
        assert!(parse("W:Wa9:B1", &e).is_err()); // rank above board
        assert!(parse("W:Wz1:B1", &e).is_err()); // file beyond board
        assert!(parse("W:Wa2:B1", &e).is_err()); // not a playing square
        assert!(parse("W:WK:B1", &e).is_err()); // prefix without square
    }
}
