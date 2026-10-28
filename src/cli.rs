// CLI: `boardimage render` — render to a file without the server (SPEC.md).
// Example:
//   boardimage render --fen "W:W31-50:B1-20" --out start.png
//   boardimage render --game-type 25 --fen ... --notation numeric --out r.png
//   boardimage render --fen a.fen --fen b.fen --out replay.gif --delay-ms 800

use crate::board;
use crate::gif::{Frame, GifParams};
use crate::render::{png, svg, Caption, Notation, Params};
use crate::{fen, theme};

pub fn run_render(args: &[String]) -> Result<(), String> {
    let get = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let out = get("--out").ok_or("--out <file> required")?;
    let fens: Vec<String> = args
        .windows(2)
        .filter(|w| w[0] == "--fen")
        .filter_map(|w| w.get(1).cloned())
        .collect();
    if fens.is_empty() {
        return Err("--fen <FEN> required (repeat for a GIF)".into());
    }
    let size: u32 = get("--size")
        .and_then(|s| s.parse().ok())
        .unwrap_or(512)
        .clamp(16, 1024);
    let (bdef, _) = get("--game-type")
        .and_then(|g| g.parse().ok())
        .and_then(board::from_game_type)
        .or_else(|| get("--variant").and_then(|v| board::from_variant(&v)))
        .unwrap_or((
            board::BoardDef {
                width: 10,
                height: 10,
                light_squares: false,
                numbering: 0,
            },
            "standard",
        ));
    let th = get("--theme")
        .and_then(|t| theme::by_name(&t))
        .unwrap_or_else(theme::default_theme);
    let last_move = get("--last-move").and_then(|s| parse_lm(&s, bdef.max_square()));
    let caption = {
        let w = get("--white").unwrap_or_default();
        let b = get("--black").unwrap_or_default();
        (!w.is_empty() || !b.is_empty()).then(|| Caption {
            white: w,
            black: b,
            comment: get("--comment").unwrap_or_default(),
        })
    };
    let notation = match get("--notation").as_deref() {
        Some("numeric") => Notation::Numeric,
        Some("algebraic") => Notation::Algebraic,
        Some("coords") => Notation::Coords,
        _ => Notation::None,
    };

    let ext = out.rsplit('.').next().unwrap_or("");
    match ext {
        "gif" => {
            let frames: Vec<Frame> = fens
                .into_iter()
                .map(|f| Frame { fen: f, last_move })
                .collect();
            let data = crate::gif::encode(
                &GifParams {
                    board: bdef,
                    size: size.min(640),
                    theme: th,
                    flat: args.contains(&"--flat".to_string()),
                    flipped: args.contains(&"--flip".to_string()),
                    delay_ms: get("--delay-ms")
                        .and_then(|d| d.parse().ok())
                        .unwrap_or(700),
                    caption,
                    comment: get("--comment").unwrap_or_default(),
                },
                &frames,
            )?;
            std::fs::write(&out, data).map_err(|e| e.to_string())?;
        }
        "svg" => {
            let pos = fen::parse(fens.first().unwrap(), bdef.max_square())?;
            let p = mk_params(pos, bdef, size, th, last_move, notation, caption, args);
            std::fs::write(&out, svg(&p)).map_err(|e| e.to_string())?;
        }
        _ => {
            let pos = fen::parse(fens.first().unwrap(), bdef.max_square())?;
            let p = mk_params(pos, bdef, size, th, last_move, notation, caption, args);
            std::fs::write(&out, png(&p)?).map_err(|e| e.to_string())?;
        }
    }
    println!("wrote {out}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn mk_params(
    pos: fen::Position,
    bdef: board::BoardDef,
    size: u32,
    th: &'static theme::Theme,
    last_move: Option<(u16, u16)>,
    notation: Notation,
    caption: Option<Caption>,
    args: &[String],
) -> Params {
    Params {
        pos,
        board: bdef,
        size,
        theme: th,
        flat: args.contains(&"--flat".to_string()),
        flipped: args.contains(&"--flip".to_string()),
        last_move,
        squares: vec![],
        arrows: vec![],
        notation,
        margin: 0,
        transparent: args.contains(&"--transparent".to_string()),
        caption,
    }
}

fn parse_lm(s: &str, max: u16) -> Option<(u16, u16)> {
    if s.len() < 4 || s.len() > 6 || !s.len().is_multiple_of(2) {
        return None;
    }
    let h = s.len() / 2;
    let a: u16 = s[..h].parse().ok()?;
    let b: u16 = s[h..].parse().ok()?;
    (a >= 1 && b >= 1 && a <= max && b <= max).then_some((a, b))
}
