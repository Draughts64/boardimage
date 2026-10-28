// Golden renders + file-size bounds (SPEC.md §10, task 6/8a).

use boardimage::render::{png, svg, Notation, Params};
use boardimage::{board, fen, theme};

fn params(fen: &str, gt: u32, size: u32) -> Params {
    let (bdef, _) = board::from_game_type(gt).unwrap();
    Params {
        pos: fen::parse(fen, &bdef).unwrap(),
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
    walk_gif_blocks(&data);
}

#[test]
fn golden_gif_comment_is_structurally_valid() {
    use boardimage::gif::{encode, Frame, GifParams};
    let (bdef, _) = boardimage::board::from_game_type(20).unwrap();
    let frames: Vec<Frame> = (0..3)
        .map(|i| Frame {
            fen: if i % 2 == 0 {
                "W:W31-50:B1-20".into()
            } else {
                "W:W31-49:B1-20".into()
            },
            last_move: None,
        })
        .collect();
    let data = encode(
        &GifParams {
            board: bdef,
            size: 256,
            theme: theme::default_theme(),
            flat: false,
            flipped: false,
            delay_ms: 500,
            caption: None,
            comment: "Tournament 2026".into(),
        },
        &frames,
    )
    .unwrap();
    // the comment must sit AFTER the Logical Screen Descriptor (+GCT),
    // never between the header and the LSD (that corrupts the file)
    let comment_at = data
        .windows(2)
        .position(|w| w == [0x21, 0xFE])
        .expect("comment extension present");
    let packed = data[10];
    let lsd_end = 6
        + 7
        + if packed & 0x80 != 0 {
            3 * (1usize << ((packed & 0x07) + 1))
        } else {
            0
        };
    assert!(
        comment_at >= lsd_end,
        "comment at {comment_at} < lsd_end {lsd_end}"
    );
    walk_gif_blocks(&data);
}

/// Walk every GIF block; the file must parse cleanly to the trailer
/// (frames, delays and extensions all reachable).
fn walk_gif_blocks(data: &[u8]) {
    assert_eq!(&data[..6], b"GIF89a");
    let packed = data[10];
    let gct = if packed & 0x80 != 0 {
        3 * (1usize << ((packed & 0x07) + 1))
    } else {
        0
    };
    let mut p = 6 + 7 + gct;
    let mut frames = 0usize;
    let mut comments = 0usize;
    while p < data.len() {
        match data[p] {
            0x21 => {
                let label = data[p + 1];
                let mut q = p + 2;
                while data[q] != 0 {
                    q += 1 + data[q] as usize;
                }
                comments += usize::from(label == 0xFE);
                p = q + 1;
            }
            0x2C => {
                frames += 1;
                let lct_packed = data[p + 9];
                let mut q = p + 10;
                if lct_packed & 0x80 != 0 {
                    q += 3 * (1usize << ((lct_packed & 0x07) + 1));
                }
                q += 1; // LZW min code size
                while data[q] != 0 {
                    q += 1 + data[q] as usize;
                }
                p = q + 1;
            }
            0x3B => break, // trailer
            other => panic!("invalid GIF block 0x{other:02x} at offset {p}"),
        }
    }
    assert_eq!(data[p], 0x3B, "must end at the trailer");
    assert!(frames >= 1);
    assert!(comments <= 2, "ours + gifski's own watermark");
}
