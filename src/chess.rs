// Chess FEN per the PGN standard §16.1.3 (Steven J. Edwards) —
// see README links. Six space-separated fields; the last two counters
// may be omitted (EPD-style, tolerated like draughts :H0:F1).
//
//   rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
//
// Castling and en-passant fields are format-validated only (this is a
// renderer, not a rules engine); Shredder-FEN file letters (HAha) are
// accepted alongside KQkq.

use crate::render::BoardPiece;

/// The chess board: 8x8, every square numbered bottom-up, left-to-right
/// (rank 1 = squares 1..8, so e2 = 13, a8 = 57).
pub fn board() -> crate::board::BoardDef {
    crate::board::BoardDef {
        width: 8,
        height: 8,
        light_squares: false,
        numbering: 0,
        all_squares: true,
    }
}

fn tag_for(letter: u8) -> Option<(&'static str, bool)> {
    Some(match letter {
        b'K' => ("wK", true),
        b'Q' => ("wQ", true),
        b'R' => ("wR", true),
        b'B' => ("wB", true),
        b'N' => ("wN", true),
        b'P' => ("wP", true),
        b'k' => ("bK", false),
        b'q' => ("bQ", false),
        b'r' => ("bR", false),
        b'b' => ("bB", false),
        b'n' => ("bN", false),
        b'p' => ("bP", false),
        _ => return None,
    })
}

pub fn parse(fen: &str) -> Result<Vec<BoardPiece>, &'static str> {
    let fields: Vec<&str> = fen.split_whitespace().collect();
    if fields.len() < 4 {
        return Err("invalid fen");
    }
    let mut pieces = Vec::new();
    let ranks: Vec<&str> = fields[0].split('/').collect();
    if ranks.len() != 8 {
        return Err("invalid fen");
    }
    for (i, rank) in ranks.iter().enumerate() {
        let rank_no = 8 - i as u16; // first FEN rank is 8
        let mut file: u16 = 0;
        for &b in rank.as_bytes() {
            match b {
                b'1'..=b'8' => file += (b - b'0') as u16,
                _ => {
                    let Some((tag, white)) = tag_for(b) else {
                        return Err("invalid fen");
                    };
                    if file >= 8 {
                        return Err("invalid fen");
                    }
                    pieces.push(BoardPiece {
                        square: (rank_no - 1) * 8 + file + 1,
                        white,
                        king: false,
                        ghost: false,
                        tag,
                    });
                    file += 1;
                }
            }
        }
        if file != 8 {
            return Err("invalid fen"); // rank must sum to 8 squares
        }
    }
    if !matches!(fields[1], "w" | "b") {
        return Err("invalid fen");
    }
    if !valid_castling(fields[2]) {
        return Err("invalid fen");
    }
    if fields[3] != "-" && !valid_ep(fields[3]) {
        return Err("invalid fen");
    }
    if fields.len() >= 5 && fields[4].parse::<u32>().is_err() {
        return Err("invalid fen");
    }
    if fields.len() >= 6 && (fields[5].parse::<u32>().is_err() || fields[5] == "0") {
        return Err("invalid fen"); // fullmove starts at 1
    }
    Ok(pieces)
}

fn valid_castling(s: &str) -> bool {
    if s == "-" {
        return true;
    }
    // KQkq order is conventional but tools shuffle; accept any subset
    let mut seen = [false; 4];
    for c in s.bytes() {
        let idx = match c {
            b'K' => 0,
            b'Q' => 1,
            b'k' => 2,
            b'q' => 3,
            // Shredder-FEN: the rook files (A..H / a..h)
            b'A'..=b'H' | b'a'..=b'h' => continue,
            _ => return false,
        };
        if seen[idx] {
            return false;
        }
        seen[idx] = true;
    }
    true
}

fn valid_ep(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 2 && (b'a'..=b'h').contains(&b[0]) && (b[1] == b'3' || b[1] == b'6')
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn start_position() {
        let p = parse(START).unwrap();
        assert_eq!(p.len(), 32);
        // e2 = square 13, white pawn; e7 = 53, black pawn; a8 = 57 black rook
        assert!(p.iter().any(|pc| pc.square == 13 && pc.tag == "wP"));
        assert!(p.iter().any(|pc| pc.square == 53 && pc.tag == "bP"));
        assert!(p
            .iter()
            .any(|pc| pc.square == 57 && pc.tag == "bR" && pc.white == false));
        assert!(p.iter().any(|pc| pc.square == 5 && pc.tag == "wK")); // e1
    }

    #[test]
    fn spec_example_after_e4_c5() {
        let fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq c6 0 2";
        let p = parse(fen).unwrap();
        assert_eq!(p.len(), 32); // two pawns advanced, none captured
        assert!(p.iter().any(|pc| pc.square == 29 && pc.tag == "wP")); // e4
        assert!(p.iter().any(|pc| pc.square == 35 && pc.tag == "bP")); // c5
    }

    #[test]
    fn epd_without_counters_tolerated() {
        let p = parse("4k3/8/8/8/8/8/8/4K3 w - -").unwrap();
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn shredder_castling_tolerated() {
        assert!(parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w AHah - 0 1").is_ok());
    }

    #[test]
    fn kiwipete() {
        // classic perft test position (complex castling/ep/queens)
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let p = parse(fen).unwrap();
        assert_eq!(p.len(), 32);
    }

    #[test]
    fn errors() {
        assert!(parse("").is_err());
        assert!(
            parse("8/8/8/8/8/8/8/8 w - - 0 1").is_ok(),
            "no kings is fine for a renderer"
        );
        assert!(parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP w KQkq - 0 1").is_err()); // 7 ranks
        assert!(parse("rnbqkbnr/pppppppp/9/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").is_err()); // 9 files
        assert!(parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1").is_err());
        assert!(parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQxq - 0 1").is_err());
        assert!(parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e9 0 1").is_err());
        assert!(parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - x 1").is_err());
    }
}
