// Golden renders + file-size bounds (SPEC.md §10, task 6/8a).

use boardimage::render::{png, svg, Notation, Params};
use boardimage::{board, fen, theme};

fn params(fen: &str, gt: u32, size: u32) -> Params {
    let (bdef, _) = board::from_game_type(gt).unwrap();
    Params {
        pos: fen::parse(fen, bdef.max_square()).unwrap(),
        board: bdef,
        size,
        theme: theme::default_theme(),
        flat: false,
        flipped: false,
        last_move: None,
        squares: vec![],
        arrows: vec![],
        notation: Notation::None,
        margin: 0,
        transparent: false,
        caption: None,
    }
}

#[test]
fn golden_png_signature_and_bounds() {
    let p = params("W:W31-50:B1-20", 20, 1024);
    let data = png(&p).unwrap();
    assert_eq!(&data[..8], b"\x89PNG\r\n\x1a\n");
    assert!(data.len() < 250 * 1024, "PNG@1024 too big: {}B", data.len());
}

#[test]
fn golden_russian_kings_lastmove() {
    let mut p = params("W:W21,K22:B1,2", 25, 512);
    p.last_move = Some((22, 18));
    let data = png(&p).unwrap();
    assert!(data.len() < 120 * 1024);
}

#[test]
fn golden_italian_light_squares() {
    let p = params("W:W21-32:B1-12", 22, 512);
    let data = png(&p).unwrap();
    assert!(data.len() > 10 * 1024, "suspiciously small render");
}

#[test]
fn golden_canadian_12x12() {
    let p = params("W:W43-72:B1-30", 27, 640);
    let data = png(&p).unwrap();
    assert!(data.len() > 20 * 1024);
}

#[test]
fn golden_svg_notation() {
    let mut p = params("W:W31-50:B1-20", 20, 512);
    p.notation = Notation::Numeric;
    let s = svg(&p);
    assert!(s.starts_with("<svg"));
    assert!(
        s.matches("<text").count() >= 50,
        "every playing square gets its number"
    );
}

#[test]
fn golden_gif_sizes() {
    use boardimage::gif::{encode, Frame, GifParams};
    let (bdef, _) = board::from_game_type(20).unwrap();
    let frames: Vec<Frame> = (0..30)
        .map(|i| Frame {
            fen: if i % 2 == 0 {
                "W:W31-50:B1-20".into()
            } else {
                "W:W31-49:B1-20".into()
            },
            last_move: (i % 2 == 1).then_some((47, 41)),
        })
        .collect();
    let data = encode(
        &GifParams {
            board: bdef,
            size: 512,
            theme: theme::default_theme(),
            flat: false,
            flipped: false,
            delay_ms: 500,
            caption: None,
            comment: String::new(),
        },
        &frames,
    )
    .unwrap();
    assert_eq!(&data[..6], b"GIF89a");
    assert!(data.len() < 500 * 1024, "GIF too big: {}B", data.len());
}
