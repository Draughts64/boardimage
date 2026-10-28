// Rendering: raster (tiny-skia) and SVG from one geometry (SPEC.md §4).

use crate::board::BoardDef;
use crate::fen::{Kind, Position};
use crate::glyph;
use crate::theme::Theme;
use tiny_skia::*;

#[derive(Clone)]
pub struct Arrow {
    pub from: u16,
    pub to: u16,
    pub color: Option<char>, // G/B/R/Y
}

#[derive(Clone, Default)]
pub struct Caption {
    pub white: String,
    pub black: String,
    pub comment: String,
}

#[derive(Clone)]
pub struct Params {
    pub pos: Position,
    pub board: BoardDef,
    pub size: u32, // square side in px
    pub theme: &'static Theme,
    pub flat: bool,    // pieceStyle=flat
    pub flipped: bool, // orientation=black
    pub last_move: Option<(u16, u16)>,
    pub squares: Vec<(u16, f32)>, // heatmap (alpha 0..1)
    pub arrows: Vec<Arrow>,
    pub notation: Notation,
    pub margin: u32,
    pub transparent: bool,
    pub caption: Option<Caption>,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Notation {
    #[default]
    None,
    Numeric,
    Algebraic,
    Coords,
}

fn arrow_rgba(c: Option<char>) -> ([u8; 4], &'static str) {
    match c {
        Some('G') => ([0x41, 0xa6, 0x3c, 0xb4], "#41a63c"),
        Some('B') => ([0x38, 0x97, 0xd3, 0xb4], "#3897d3"),
        Some('R') => ([0xd3, 0x3d, 0x38, 0xb4], "#d33d38"),
        Some('Y') => ([0xd8, 0xc0, 0x3a, 0xb4], "#d8c03a"),
        _ => ([0x33, 0x33, 0x33, 0xa0], "#333333"),
    }
}

impl Params {
    fn caption_h(&self) -> u32 {
        if self
            .caption
            .as_ref()
            .is_some_and(|c| !c.white.is_empty() || !c.black.is_empty())
        {
            self.size / 6
        } else {
            0
        }
    }
    fn total_h(&self) -> u32 {
        self.margin * 2 + self.size + self.caption_h()
    }
    fn total_w(&self) -> u32 {
        self.margin * 2 + self.size
    }
    fn cell(&self) -> f32 {
        self.size as f32 / self.board.width as f32
    }
    /// screen coords (top-left origin) of cell (x,y from bottom-left)
    fn sx(&self, x: u16) -> f32 {
        let mx = if self.flipped {
            self.board.width - 1 - x
        } else {
            x
        };
        self.margin as f32 + mx as f32 * self.cell()
    }
    fn sy(&self, y: u16) -> f32 {
        let my = if self.flipped {
            self.board.height - 1 - y
        } else {
            y
        };
        self.margin as f32 + (self.board.height - 1 - my) as f32 * self.cell()
    }
    fn center(&self, x: u16, y: u16) -> (f32, f32) {
        let s = self.cell();
        (self.sx(x) + s / 2.0, self.sy(y) + s / 2.0)
    }
}

// ── raster ─────────────────────────────────────────────────────────────

pub fn raster(p: &Params) -> Pixmap {
    let w = p.total_w();
    let h = p.total_h();
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("pixmap");
    draw_board_raster(p, &mut pm);
    draw_pieces_raster(p, &mut pm);
    draw_overlays_raster(p, &mut pm);
    if let Some(cap) = &p.caption {
        draw_caption_raster(p, cap, &mut pm);
    }
    pm
}

fn paint(c: [u8; 3], a: u8) -> Paint<'static> {
    let mut pt = Paint::default();
    pt.set_color_rgba8(c[0], c[1], c[2], a);
    pt.anti_alias = true;
    pt
}

fn rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, pt: &Paint) {
    let r = Rect::from_xywh(x, y, w, h).expect("rect");
    pm.fill_rect(r, pt, Transform::identity(), None);
}

fn disc_path(cx: f32, cy: f32, r: f32) -> Path {
    let k = 0.5523 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
    pb.cubic_to(cx - r, cy + k, cx - r, cy + k, cx - r, cy);
    pb.push_circle(cx, cy, r); // ensure closure
    pb.finish().expect("path")
}

fn stroke_path(pm: &mut Pixmap, path: &Path, color: [u8; 3], w: f32) {
    let s = Stroke {
        width: w,
        ..Default::default()
    };
    pm.stroke_path(path, &paint(color, 255), &s, Transform::identity(), None);
}

fn draw_board_raster(p: &Params, pm: &mut Pixmap) {
    let c = p.cell();
    if !p.transparent {
        let bg = p.theme.light;
        rect(
            pm,
            p.margin as f32,
            p.margin as f32,
            p.size as f32,
            p.size as f32,
            &paint(bg, 255),
        );
    }
    let last = p.last_move;
    for y in 0..p.board.height {
        for x in 0..p.board.width {
            if !p.board.is_playing(x, y) {
                continue;
            }
            let is_last = last.is_some_and(|(a, b)| {
                p.board.xy(a) == Some((x, y)) || p.board.xy(b) == Some((x, y))
            });
            let (col, alpha) = if is_last {
                (
                    if p.board.is_playing(x, y) && (x + y) % 2 == 0 {
                        p.theme.last_dark
                    } else {
                        p.theme.last_light
                    },
                    255,
                )
            } else {
                // playing squares on the "dark" visual phase
                let dark_visual = (x + y) % 2 == 0;
                let col = if dark_visual {
                    p.theme.dark
                } else {
                    p.theme.light
                };
                (col, 255)
            };
            rect(pm, p.sx(x), p.sy(y), c + 0.5, c + 0.5, &paint(col, alpha));
        }
    }
    // heatmap squares (over the base fill)
    for (sq, a) in &p.squares {
        if let Some((x, y)) = p.board.xy(*sq) {
            rect(
                pm,
                p.sx(x),
                p.sy(y),
                c + 0.5,
                c + 0.5,
                &paint([0x21, 0x96, 0xf3], (a.clamp(0.0, 1.0) * 200.0) as u8),
            );
        }
    }
}

fn draw_piece(pm: &mut Pixmap, p: &Params, white: bool, kind: Kind, cx: f32, cy: f32, s: f32) {
    let base = s * 0.38;
    let alpha = if kind.is_ghost() { 76 } else { 255 };
    let (body, edge) = if white {
        ([255, 255, 255], [0, 0, 0])
    } else {
        ([0x1a, 0x1a, 0x1a], [0xdf, 0xdf, 0xdf])
    };
    // stack of discs: man = 2, king = 3 (visual height via offsets)
    let rings: &[f32] = if kind.is_king() {
        &[0.00, -0.07, -0.14]
    } else {
        &[0.00, -0.06]
    };
    if p.flat {
        let mut path = disc_path(cx, cy, base);
        for &o in rings.iter().skip(1) {
            let d = disc_path(cx, cy + o * s, base);
            path = path.clone(); // keep simple: draw separately
            let pt = paint(body, alpha);
            pm.fill_path(&d, &pt, FillRule::Winding, Transform::identity(), None);
        }
        pm.fill_path(
            &path,
            &paint(body, alpha),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        stroke_path(pm, &disc_path(cx, cy, base), edge, s * 0.012);
    } else {
        // classic: gradient approximated by layered discs (bottom ring darker)
        for (i, &o) in rings.iter().enumerate() {
            let shade = if white && i == 0 {
                [0x7f, 0x7f, 0x7f]
            } else if !white && i == 0 {
                [0x00, 0x00, 0x00]
            } else {
                body
            };
            let d = disc_path(cx, cy + o * s, base);
            pm.fill_path(
                &d,
                &paint(shade, alpha),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
            stroke_path(pm, &d, edge, s * 0.012);
        }
        // top cap for white kings
        if kind.is_king() && white {
            let d = disc_path(cx, cy - 0.14 * s, base);
            pm.fill_path(
                &d,
                &paint(body, alpha),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    if kind.is_king() {
        // crown: gold star polygon
        let r = base * 0.62;
        let mut pb = PathBuilder::new();
        for i in 0..10 {
            let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rad = if i % 2 == 0 { r } else { r * 0.45 };
            let (px, py) = (cx + rad * ang.cos(), cy + rad * ang.sin() - s * 0.05);
            if i == 0 {
                pb.move_to(px, py)
            } else {
                pb.line_to(px, py)
            };
        }
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(
                &path,
                &paint([0xbf, 0x8c, 0x16], alpha),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

fn draw_pieces_raster(p: &Params, pm: &mut Pixmap) {
    let s = p.cell();
    let mut items: Vec<_> = p.pos.pieces().collect();
    items.sort_by_key(|(_, piece)| piece.square);
    for (white, piece) in items {
        if let Some((x, y)) = p.board.xy(piece.square) {
            let (cx, cy) = p.center(x, y);
            draw_piece(pm, p, white, piece.kind, cx, cy, s);
        }
    }
}

fn draw_overlays_raster(p: &Params, pm: &mut Pixmap) {
    let s = p.cell();
    // arrows
    for a in &p.arrows {
        let (rgba, _) = arrow_rgba(a.color);
        let Some((ax, ay)) = p.board.xy(a.from) else {
            continue;
        };
        let Some((bx, by)) = p.board.xy(a.to) else {
            continue;
        };
        let (fx, fy) = p.center(ax, ay);
        let (tx, ty) = p.center(bx, by);
        if a.from == a.to {
            let d = disc_path(fx, fy, s * 0.16);
            pm.fill_path(
                &d,
                &paint([rgba[0], rgba[1], rgba[2]], rgba[3]),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
            continue;
        }
        let (dx, dy) = (tx - fx, ty - fy);
        let len = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / len, dy / len);
        let head = s * 0.30;
        let start_off = s * 0.34;
        let mut pb = PathBuilder::new();
        pb.move_to(fx + ux * start_off, fy + uy * start_off);
        pb.line_to(tx - ux * head, ty - uy * head);
        let shaft = pb.finish().unwrap();
        let st = Stroke {
            width: s * 0.14,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        pm.stroke_path(
            &shaft,
            &paint([rgba[0], rgba[1], rgba[2]], rgba[3]),
            &st,
            Transform::identity(),
            None,
        );
        // arrowhead diamond
        let mut pb = PathBuilder::new();
        let (nx, ny) = (-uy, ux);
        pb.move_to(tx, ty);
        pb.line_to(
            tx - ux * head + nx * head * 0.5,
            ty - uy * head + ny * head * 0.5,
        );
        pb.line_to(tx - ux * head * 0.4, ty - uy * head * 0.4);
        pb.line_to(
            tx - ux * head - nx * head * 0.5,
            ty - uy * head - ny * head * 0.5,
        );
        pb.close();
        if let Some(h) = pb.finish() {
            pm.fill_path(
                &h,
                &paint([rgba[0], rgba[1], rgba[2]], rgba[3]),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    // notation
    match p.notation {
        Notation::Numeric => {
            let fs = (s * 0.22).clamp(6.0, 22.0);
            for (n, x, y) in p.board.xy_iter() {
                let (sx, sy) = (p.sx(x) + s * 0.06, p.sy(y) + s * 0.06);
                let dark_visual = (x + y) % 2 == 0;
                let col = if dark_visual {
                    [0xff, 0xff, 0xff]
                } else {
                    [0x44, 0x44, 0x44]
                };
                draw_text(pm, &n.to_string(), sx, sy, fs * 0.55, col);
            }
        }
        Notation::Coords => {
            let fs = (s * 0.20).clamp(6.0, 20.0);
            for x in 0..p.board.width {
                for y in 0..p.board.height {
                    let label = p.board.algebraic(x, y);
                    let (sx, sy) = (p.sx(x) + s * 0.06, p.sy(y) + s * 0.06);
                    let dark_visual = (x + y) % 2 == 0;
                    let col = if dark_visual {
                        [0xff, 0xff, 0xff]
                    } else {
                        [0x55, 0x55, 0x55]
                    };
                    draw_text(pm, &label, sx, sy, fs * 0.55, col);
                }
            }
        }
        Notation::Algebraic => {
            let fs = (s * 0.24).clamp(7.0, 26.0);
            for x in 0..p.board.width {
                let letter = p.board.file_letter(x);
                let sx = p.sx(x) + s / 2.0 - fs * 0.35;
                let sy = if p.flipped {
                    p.margin as f32 + p.size as f32 + fs * 0.9
                } else {
                    p.margin as f32 - fs * 0.25
                };
                draw_text(
                    pm,
                    &letter.to_string(),
                    sx,
                    sy,
                    fs * 0.6,
                    [0x33, 0x33, 0x33],
                );
            }
            for y in 0..p.board.height {
                let sx = if p.flipped {
                    p.margin as f32 - fs * 1.6
                } else {
                    p.margin as f32 + p.size as f32 + fs * 0.4
                };
                let sy = p.sy(y) + s / 2.0 - fs * 0.3;
                draw_text(
                    pm,
                    &(y + 1).to_string(),
                    sx,
                    sy,
                    fs * 0.6,
                    [0x33, 0x33, 0x33],
                );
            }
        }
        Notation::None => {}
    }
}

/// Cut the string (with an ellipsis) so its rendered width fits max_px.
fn truncate(s: &str, max_px: f32, px: f32) -> String {
    if glyph::text_width(s) as f32 * px <= max_px {
        return s.to_string();
    }
    let mut out: String = s.chars().take(2).collect();
    for c in s.chars().skip(2) {
        let t = format!("{out}{c}");
        if glyph::text_width(&t) as f32 * px > max_px - px {
            return format!("{out}.");
        }
        out = t;
    }
    out
}

fn draw_text(pm: &mut Pixmap, s: &str, x: f32, y: f32, px: f32, color: [u8; 3]) {
    // 3x5 bitmap glyphs; px = pixel size of one bitmap pixel
    for (gx, gy) in glyph::text_pixels(s) {
        rect(
            pm,
            x + gx as f32 * px,
            y + gy as f32 * px,
            px + 0.5,
            px + 0.5,
            &paint(color, 190),
        );
    }
}

fn draw_caption_raster(p: &Params, cap: &Caption, pm: &mut Pixmap) {
    let h = p.caption_h() as f32;
    let w = p.total_w() as f32;
    rect(pm, 0.0, 0.0, w, h, &paint([0x26, 0x24, 0x21], 255));
    // 3x5 glyphs: one bitmap pixel is `px`, so a char is 5*px tall —
    // the bar must fit a names row AND a smaller comment row below it
    let px = (h / 18.0).clamp(1.0, 12.0);
    let half = w / 2.0 - px * 2.0;
    if !cap.white.is_empty() || !cap.black.is_empty() {
        let white = truncate(&cap.white, half, px);
        let black = truncate(&cap.black, half, px);
        draw_text(
            pm,
            &white,
            p.margin as f32,
            h * 0.16,
            px,
            [0xff, 0xff, 0xff],
        );
        let bw = glyph::text_width(&black) as f32 * px;
        draw_text(
            pm,
            &black,
            w - p.margin as f32 - bw,
            h * 0.16,
            px,
            [0xff, 0xff, 0xff],
        );
    }
    if !cap.comment.is_empty() {
        let comment = truncate(&cap.comment, w - px * 4.0, px * 0.8);
        let cw = glyph::text_width(&comment) as f32 * px * 0.8;
        draw_text(
            pm,
            &comment,
            (w - cw) / 2.0,
            h * 0.60,
            px * 0.8,
            [0xbb, 0xb3, 0xa8],
        );
    }
}

// ── SVG ────────────────────────────────────────────────────────────────

pub fn svg(p: &Params) -> String {
    let c = p.cell();
    let w = p.total_w();
    let h = p.total_h();
    let mut o = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"##
    );
    if let Some(cap) = &p.caption {
        let ch = p.caption_h();
        o += &format!(r##"<rect width="{w}" height="{ch}" fill="#262421"/>"##);
        let px = (ch as f32 / 18.0).clamp(1.0, 12.0);
        let half = w as f32 / 2.0 - px * 2.0;
        if !cap.white.is_empty() {
            let white = truncate(&cap.white, half, px);
            o += &svg_text(&white, p.margin as f32, ch as f32 * 0.16, px * 5.0, None);
        }
        if !cap.black.is_empty() {
            let black = truncate(&cap.black, half, px);
            let bw = glyph::text_width(&black) as f32 * px * 1.25;
            o += &svg_text(
                &black,
                w as f32 - p.margin as f32 - bw,
                ch as f32 * 0.16,
                px * 5.0,
                None,
            );
        }
        if !cap.comment.is_empty() {
            let comment = truncate(&cap.comment, w as f32 - px * 4.0, px * 0.8);
            let cw = glyph::text_width(&comment) as f32 * px * 1.0;
            o += &svg_text(
                &comment,
                (w as f32 - cw) / 2.0,
                ch as f32 * 0.60,
                px * 4.0,
                Some("#bbb3a8"),
            );
        }
    }
    if !p.transparent {
        o += &format!(
            r##"<rect x="{m}" y="{m}" width="{s}" height="{s}" fill="{}"/>"##,
            crate::theme::hex(p.theme.light),
            m = p.margin,
            s = p.size
        );
    }
    for y in 0..p.board.height {
        for x in 0..p.board.width {
            if !p.board.is_playing(x, y) {
                continue;
            }
            let is_last = p.last_move.is_some_and(|(a, b)| {
                p.board.xy(a) == Some((x, y)) || p.board.xy(b) == Some((x, y))
            });
            let dark_visual = (x + y) % 2 == 0;
            let col = if is_last {
                if dark_visual {
                    p.theme.last_dark
                } else {
                    p.theme.last_light
                }
            } else if dark_visual {
                p.theme.dark
            } else {
                p.theme.light
            };
            o += &format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"##,
                fmt(p.sx(x)),
                fmt(p.sy(y)),
                fmt(c + 0.5),
                fmt(c + 0.5),
                crate::theme::hex(col)
            );
        }
    }
    for (sq, a) in &p.squares {
        if let Some((x, y)) = p.board.xy(*sq) {
            o += &format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#2196f3" fill-opacity="{:.2}"/>"##,
                fmt(p.sx(x)),
                fmt(p.sy(y)),
                fmt(c),
                fmt(c),
                a.clamp(0.0, 1.0) * 0.8
            );
        }
    }
    let mut items: Vec<_> = p.pos.pieces().collect();
    items.sort_by_key(|(_, piece)| piece.square);
    for (white, piece) in items {
        let Some((x, y)) = p.board.xy(piece.square) else {
            continue;
        };
        let (cx, cy) = p.center(x, y);
        let r = c * 0.38;
        let (fill, stroke) = if white {
            ("#ffffff", "#000000")
        } else {
            ("#1a1a1a", "#dfdfdf")
        };
        let op = if piece.kind.is_ghost() {
            " opacity=\"0.3\""
        } else {
            ""
        };
        o += &format!(r#"<g{op}>"#);
        let rings: &[f32] = if piece.kind.is_king() {
            &[0.0, -0.07, -0.14]
        } else {
            &[0.0, -0.06]
        };
        for &ro in rings {
            o += &format!(
                r##"<circle cx="{}" cy="{}" r="{}" fill="{fill}" stroke="{stroke}" stroke-width="{}"/>"##,
                fmt(cx),
                fmt(cy + ro * c),
                fmt(r),
                fmt(c * 0.012)
            );
        }
        if piece.kind.is_king() {
            let (cx2, cy2, _rr) = (fmt(cx), fmt(cy - c * 0.05), fmt(r * 0.62));
            o += &format!(r##"<polygon fill="#bf8c16" points="{cx2},{cy2} "/>"##); // placeholder crown replaced below
            let mut pts = String::new();
            for i in 0..10 {
                let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rad = if i % 2 == 0 { r * 0.62 } else { r * 0.28 };
                pts += &format!(
                    "{},{} ",
                    fmt(cx + rad * ang.cos()),
                    fmt(cy - c * 0.05 + rad * ang.sin())
                );
            }
            o += &format!(r##"<polygon fill="#bf8c16" points="{pts}"/>"##);
        }
        o += "</g>";
    }
    for a in &p.arrows {
        let (_, hexc) = arrow_rgba(a.color);
        let Some((ax, ay)) = p.board.xy(a.from) else {
            continue;
        };
        let Some((bx, by)) = p.board.xy(a.to) else {
            continue;
        };
        let (fx, fy) = p.center(ax, ay);
        let (tx, ty) = p.center(bx, by);
        if a.from == a.to {
            o += &format!(
                r##"<circle cx="{}" cy="{}" r="{}" fill="{hexc}" fill-opacity="0.7"/>"##,
                fmt(fx),
                fmt(fy),
                fmt(c * 0.16)
            );
        } else {
            o += &format!(
                r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{hexc}" stroke-width="{}" stroke-opacity="0.7" stroke-linecap="round"/>"##,
                fmt(fx),
                fmt(fy),
                fmt(tx),
                fmt(ty),
                fmt(c * 0.14)
            );
        }
    }
    match p.notation {
        Notation::Numeric => {
            for (n, x, y) in p.board.xy_iter() {
                let col = if (x + y) % 2 == 0 { "#fff" } else { "#444" };
                o += &format!(
                    r##"<text x="{}" y="{}" font-size="{:.0}" fill="{col}" font-family="sans-serif">{}.{}</text>"##,
                    fmt(p.sx(x) + c * 0.10),
                    fmt(p.sy(y) + c * 0.30),
                    c * 0.24,
                    "",
                    n
                );
            }
        }
        Notation::Coords => {
            for x in 0..p.board.width {
                for y in 0..p.board.height {
                    let letter = (b'a' + (x as u8 % 26)) as char;
                    let col = if (x + y) % 2 == 0 { "#fff" } else { "#555" };
                    o += &format!(
                        r##"<text x="{}" y="{}" font-size="{:.0}" fill="{col}" font-family="sans-serif">{}{}</text>"##,
                        fmt(p.sx(x) + c * 0.08),
                        fmt(p.sy(y) + c * 0.28),
                        c * 0.22,
                        letter,
                        y + 1
                    );
                }
            }
        }
        Notation::Algebraic => {
            for x in 0..p.board.width {
                let letter = (b'a' + (x as u8 % 26)) as char;
                o += &format!(
                    r##"<text x="{}" y="{}" font-size="{:.0}" fill="#333" font-family="sans-serif">{}</text>"##,
                    fmt(p.sx(x) + c * 0.38),
                    fmt(p.margin as f32 - c * 0.06),
                    c * 0.26,
                    letter
                );
            }
            for y in 0..p.board.height {
                o += &format!(
                    r##"<text x="{}" y="{}" font-size="{:.0}" fill="#333" font-family="sans-serif">{}</text>"##,
                    fmt(p.margin as f32 + p.size as f32 + c * 0.10),
                    fmt(p.sy(y) + c * 0.55),
                    c * 0.26,
                    y + 1
                );
            }
        }
        Notation::None => {}
    }
    o += "</svg>";
    o
}

fn svg_text(s: &str, x: f32, y: f32, fs: f32, color: Option<&str>) -> String {
    // fs is the font size (= char height); y is the row top
    format!(
        r##"<text x="{x}" y="{}" font-size="{fs}" fill="{}" font-family="sans-serif">{}</text>"##,
        y + fs,
        color.unwrap_or("#fff"),
        escape(s)
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn fmt(v: f32) -> String {
    format!("{v:.2}")
}

// ── PNG encoding ───────────────────────────────────────────────────────

pub fn png(p: &Params) -> Result<Vec<u8>, String> {
    let pm = raster(p);
    let img = image::RgbaImage::from_raw(pm.width(), pm.height(), pm.data().to_vec())
        .ok_or("pixmap conversion")?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn webp(p: &Params, quality: u8) -> Result<Vec<u8>, String> {
    let pm = raster(p);
    let raw = pm.data().to_vec();
    let (w, h) = (pm.width(), pm.height());
    let encoder = webp::Encoder::from_rgba(&raw, w, h);
    if quality >= 100 {
        Ok(encoder.encode_lossless().to_vec())
    } else {
        // libwebp quality f32 0..100
        Ok(encoder.encode(f32::from(quality)).to_vec())
    }
}
