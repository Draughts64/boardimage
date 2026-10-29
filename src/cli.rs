// CLI: `boardimage render` — render to a file without the server.
// Examples:
//   boardimage render --fen "W:W31-50:B1-20" --out start.png
//   boardimage render --game-type 21 --fen "W:Wa1,c1:Bb8,d8" --out e.svg
//   boardimage render --game-type "21,B,8,8,N1,0" --fen ... --out e.png
//   boardimage render --variant russian --notation numeric --out r.png --fen ...
//   boardimage render --fen a.fen --fen b.fen --out replay.gif --delay-ms 800

use crate::board;
use crate::gif::{Frame, GifParams};
use crate::render::{png, svg, Caption, Notation, Params};
use crate::{fen, theme};

const VALUE_FLAGS: &[&str] = &[
    "--out",
    "--fen",
    "--size",
    "--game",
    "--game-type",
    "--variant",
    "--board-size",
    "--board-height",
    "--numbering",
    "--theme",
    "--light",
    "--dark",
    "--last-move",
    "--arrows",
    "--squares",
    "--white",
    "--black",
    "--comment",
    "--notation",
    "--margin",
    "--quality",
    "--delay-ms",
];
const BOOL_FLAGS: &[&str] = &[
    "--flip",
    "--flat",
    "--transparent",
    "--border",
    "--no-border",
    "--light-squares",
];

/// Unknown --flags are a hard error: a silently ignored typo twice
/// produced "almost right" renders (--arrows, --margin).
fn validate_args(args: &[String]) -> Result<(), String> {
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a.starts_with("--") {
            if VALUE_FLAGS.contains(&a.as_str()) {
                if i + 1 >= args.len() {
                    return Err(format!("{a} requires a value"));
                }
                i += 2;
                continue;
            }
            if BOOL_FLAGS.contains(&a.as_str()) {
                i += 1;
                continue;
            }
            return Err(format!(
                "unknown flag {a} (value flags: {} | bool flags: {})",
                VALUE_FLAGS.join(" "),
                BOOL_FLAGS.join(" ")
            ));
        }
        i += 1;
    }
    Ok(())
}

pub fn run_render(args: &[String]) -> Result<(), String> {
    validate_args(args)?;
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
    let (mut bdef, _, tag_notation) = get("--game-type")
        .and_then(|g| board::from_game_type_str(&g))
        .or_else(|| {
            get("--variant").and_then(|v| board::from_variant(&v).map(|(d, n)| (d, n, None)))
        })
        .unwrap_or((
            board::BoardDef {
                width: 10,
                height: 10,
                light_squares: false,
                numbering: 2,
                all_squares: false,
            },
            "standard",
            None,
        ));
    let bw = get("--board-size").and_then(|s| s.parse::<u16>().ok());
    let bh = get("--board-height").and_then(|s| s.parse::<u16>().ok());
    for (v, dim) in [(bw, 0u8), (bh.or(bw), 1)] {
        if let Some(v) = v.filter(|v| (2..=16).contains(v) && v.is_multiple_of(2)) {
            if dim == 0 {
                bdef.width = v;
            } else {
                bdef.height = v;
            }
        }
    }
    if let Some(n) = get("--numbering").and_then(|s| s.parse::<u8>().ok()) {
        if n <= 3 {
            bdef.numbering = n;
        }
    }
    if args.contains(&"--light-squares".to_string()) {
        bdef.light_squares = true;
    }
    if let Some(dir) = std::env::var("THEMES_DIR").ok().filter(|d| !d.is_empty()) {
        theme::load_dir(&dir);
    }
    let chess = get("--game").as_deref() == Some("chess");
    if chess {
        bdef = board::BoardDef::from(crate::chess::board());
    }
    let th = get("--theme")
        .and_then(|t| theme::by_name(&t))
        .unwrap_or_else(theme::default_theme);
    let th = match (
        get("--light").and_then(|c| theme::parse_hex(&c)),
        get("--dark").and_then(|c| theme::parse_hex(&c)),
    ) {
        (None, None) => th,
        (l, d) => theme::with_colors(th, l, d),
    };
    let last_move = get("--last-move").and_then(|s| board::parse_move_squares(&s, &bdef));
    let arrows = get("--arrows")
        .map(|s| crate::server::parse_arrows(&s, &bdef))
        .unwrap_or_default();
    let squares = get("--squares")
        .map(|s| crate::server::parse_squares(&s, &bdef))
        .unwrap_or_default();
    let caption = {
        let w = get("--white").unwrap_or_default();
        let b = get("--black").unwrap_or_default();
        (!w.is_empty() || !b.is_empty()).then(|| Caption {
            white: w,
            black: b,
            comment: get("--comment").unwrap_or_default(),
        })
    };
    let pieces_of = |fs: &str| -> Result<
        (
            Vec<crate::render::BoardPiece>,
            Option<&'static crate::theme::PieceSet>,
        ),
        String,
    > {
        if chess {
            Ok((
                crate::chess::parse(fs).map_err(|e| e.to_string())?,
                Some(crate::theme::chess_set()),
            ))
        } else {
            let pos = fen::parse(fs, &bdef)?;
            Ok((crate::render::pieces_from_fen(&pos), None))
        }
    };
    let mut margin: u32 = get("--margin")
        .and_then(|m| m.parse().ok())
        .unwrap_or(0)
        .min(64);
    let notation = match get("--notation").as_deref() {
        Some("numeric") => Notation::Numeric,
        Some("algebraic") => Notation::Algebraic,
        Some("coords") => Notation::Coords,
        _ => match tag_notation {
            // PDN GameType tag: A = algebraic labels, N/S = numeric
            Some('A') => Notation::Algebraic,
            Some('N') | Some('S') => Notation::Numeric,
            _ => Notation::None,
        },
    };

    if matches!(notation, Notation::Algebraic) && margin < 22 {
        margin = 22; // border labels need a band
    }
    let border = if args.contains(&"--border".to_string()) {
        true
    } else if args.contains(&"--no-border".to_string()) {
        false
    } else {
        margin > 0 // default: labels when there is a band
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
                    chess,
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
            let (pieces, set) = pieces_of(fens.first().unwrap())?;
            let p = mk_params(
                pieces, set, bdef, size, th, last_move, arrows, squares, notation, margin, border,
                caption, args,
            );
            std::fs::write(&out, svg(&p)).map_err(|e| e.to_string())?;
        }
        "webp" => {
            use crate::render::webp;
            let (pieces, set) = pieces_of(fens.first().unwrap())?;
            let p = mk_params(
                pieces, set, bdef, size, th, last_move, arrows, squares, notation, margin, border,
                caption, args,
            );
            let quality: u8 = get("--quality")
                .and_then(|q| q.parse().ok())
                .unwrap_or(85)
                .clamp(1, 100);
            std::fs::write(&out, webp(&p, quality)?).map_err(|e| e.to_string())?;
        }
        _ => {
            let (pieces, set) = pieces_of(fens.first().unwrap())?;
            let p = mk_params(
                pieces, set, bdef, size, th, last_move, arrows, squares, notation, margin, border,
                caption, args,
            );
            std::fs::write(&out, png(&p)?).map_err(|e| e.to_string())?;
        }
    }
    println!("wrote {out}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn mk_params(
    pieces: Vec<crate::render::BoardPiece>,
    chess: Option<&'static crate::theme::PieceSet>,
    bdef: board::BoardDef,
    size: u32,
    th: &'static theme::Theme,
    last_move: Option<(u16, u16)>,
    arrows: Vec<crate::render::Arrow>,
    squares: Vec<(u16, f32)>,
    notation: Notation,
    margin: u32,
    border: bool,
    caption: Option<Caption>,
    args: &[String],
) -> Params {
    Params {
        pieces,
        chess,
        board: bdef,
        size,
        theme: th,
        flat: args.contains(&"--flat".to_string()),
        flipped: args.contains(&"--flip".to_string()),
        last_move,
        squares,
        arrows,
        notation,
        margin,
        border,
        transparent: args.contains(&"--transparent".to_string()),
        caption,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_flag_is_rejected() {
        let args = vec!["--fen".into(), "W:W1:B2".into(), "--arrow".into()];
        assert!(run_render(&args)
            .unwrap_err()
            .contains("unknown flag --arrow"));
        assert!(
            validate_args(&["--out".to_string()]).is_err(),
            "missing value"
        );
    }

    #[test]
    fn smoke_render_writes_png() {
        let dir = std::env::temp_dir().join(format!("bicli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("s.png");
        run_render(&[
            "--fen".into(),
            "W:W31:B1".into(),
            "--out".into(),
            out.to_string_lossy().into_owned(),
            "--size".into(),
            "128".into(),
        ])
        .unwrap();
        let data = std::fs::read(&out).unwrap();
        assert_eq!(&data[..4], &[0x89, b'P', b'N', b'G']);
        std::fs::remove_dir_all(&dir).ok();
    }
}
