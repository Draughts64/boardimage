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
    /// algebraic border labels (files bottom, ranks left) in the margin band
    pub border: bool,
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

/// Draw brushes ported 1:1 from draughtsground (lidraughts analysis):
/// exact colours, opacities and line widths (units of a 512px board).
pub struct Brush {
    pub rgba: [u8; 3],
    pub opacity: f32,
    pub line_units: f32,
    pub key: &'static str,
}

pub fn brush(c: Option<char>) -> Brush {
    match c {
        Some('G') => Brush {
            rgba: [0x15, 0x78, 0x1b],
            opacity: 1.0,
            line_units: 10.0,
            key: "g",
        },
        Some('B') => Brush {
            rgba: [0x00, 0x30, 0x88],
            opacity: 1.0,
            line_units: 10.0,
            key: "b",
        },
        Some('R') => Brush {
            rgba: [0x88, 0x20, 0x20],
            opacity: 1.0,
            line_units: 10.0,
            key: "r",
        },
        Some('Y') => Brush {
            rgba: [0xe6, 0x8f, 0x00],
            opacity: 1.0,
            line_units: 10.0,
            key: "y",
        },
        // engine/auto shapes on lidraughts: paleBlue
        _ => Brush {
            rgba: [0x00, 0x30, 0x88],
            opacity: 0.4,
            line_units: 15.0,
            key: "pb",
        },
    }
}

impl Brush {
    fn alpha(&self) -> u8 {
        (self.opacity * 255.0) as u8
    }
    fn hex(&self) -> String {
        crate::theme::hex(self.rgba)
    }
}

impl Params {
    fn caption_h(&self) -> u32 {
        if self
            .caption
            .as_ref()
            .is_some_and(|c| !c.white.is_empty() || !c.black.is_empty())
        {
            self.size / 5
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
        // the caption bar sits ABOVE the board: shift everything down
        self.caption_h() as f32
            + self.margin as f32
            + (self.board.height - 1 - my) as f32 * self.cell()
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
        // the margin band shares the light colour so border labels read
        let bg = p.theme.light;
        rect(
            pm,
            0.0,
            0.0,
            p.total_w() as f32,
            p.total_h() as f32,
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
    // custom theme piece sprites (SVG/PNG from the themes directory)
    if let Some(set) = p.theme.pieces {
        let man_idx = u8::from(!white);
        let king_idx = 2 + u8::from(!white);
        let (idx, want_crown) = if kind.is_king() && set.has(king_idx) {
            (king_idx, false)
        } else {
            (man_idx, kind.is_king())
        };
        let side = (s * 0.9).ceil().max(1.0) as u32;
        if let Some(sp) = set.sprite(idx, side) {
            let paint = tiny_skia::PixmapPaint {
                opacity: if kind.is_ghost() { 0.3 } else { 1.0 },
                ..Default::default()
            };
            let x = (cx - side as f32 / 2.0).round() as i32;
            let y = (cy - side as f32 / 2.0).round() as i32;
            pm.draw_pixmap(
                x,
                y,
                sp.as_ref().as_ref(),
                &paint,
                Transform::identity(),
                None,
            );
            if want_crown {
                draw_crown(pm, cx, cy, s, base, alpha);
            }
            return;
        }
    }
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
        // classic: wide_crown-style wall approximated by layered discs
        // (bottom = gradient start, mid = lerp, top = the piece face)
        let n_rings = rings.len();
        for (i, &o) in rings.iter().enumerate() {
            let shade = match (white, i) {
                (true, i) if i + 1 < n_rings => [
                    0x7f + (0xff - 0x7f) * i as u8 / (n_rings - 1) as u8,
                    0x7f + (0xff - 0x7f) * i as u8 / (n_rings - 1) as u8,
                    0x7f + (0xff - 0x7f) * i as u8 / (n_rings - 1) as u8,
                ],
                (false, i) if i + 1 < n_rings => [
                    (0x9f * i as u8) / (n_rings - 1) as u8,
                    (0x9f * i as u8) / (n_rings - 1) as u8,
                    (0x9f * i as u8) / (n_rings - 1) as u8,
                ],
                _ => body,
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
        draw_crown(pm, cx, cy, s, base, alpha);
    }
}

fn draw_crown(pm: &mut Pixmap, cx: f32, cy: f32, s: f32, base: f32, alpha: u8) {
    // gold star polygon
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
    // arrows (draughtsground geometry: 512-unit board, marker M0,0 V4 L3,2 Z)
    for a in &p.arrows {
        let br = brush(a.color);
        let Some((ax, ay)) = p.board.xy(a.from) else {
            continue;
        };
        let Some((bx, by)) = p.board.xy(a.to) else {
            continue;
        };
        let (fx, fy) = p.center(ax, ay);
        let (tx, ty) = p.center(bx, by);
        let alpha = br.alpha();
        if a.from == a.to {
            // square circle: stroke ring hugging the cell, fill none
            // (draughtsground circleWidth: 4 units of a 512px board)
            let wpx = 4.0 * s / 51.2;
            let st = Stroke {
                width: wpx,
                line_cap: LineCap::Round,
                ..Default::default()
            };
            pm.stroke_path(
                &disc_path(fx, fy, s / 2.0 - wpx / 2.0),
                &paint(br.rgba, alpha),
                &st,
                Transform::identity(),
                None,
            );
            continue;
        }
        // shaft: round cap, width = brush units (10 user / 15 pale)
        let (dx, dy) = (tx - fx, ty - fy);
        let len = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / len, dy / len);
        let stroke_w = br.line_units * s / 51.2;
        // shorten the end by the draughtsground margin (bw/512 * board px)
        let m = p.board.width as f32 * p.board.width as f32 * s / 512.0;
        let (ex, ey) = (tx - ux * m, ty - uy * m);
        let mut pb = PathBuilder::new();
        pb.move_to(fx, fy);
        pb.line_to(ex, ey);
        let shaft = pb.finish().unwrap();
        let st = Stroke {
            width: stroke_w,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        pm.stroke_path(
            &shaft,
            &paint(br.rgba, alpha),
            &st,
            Transform::identity(),
            None,
        );
        // head: the draughtsground marker M0,0 V4 L3,2 Z in stroke units
        // (base 4w wide, 3w long, tip 0.95w past the line end, refX 2.05)
        let (nx, ny) = (-uy, ux);
        let base_c = (ex - ux * 2.05 * stroke_w, ey - uy * 2.05 * stroke_w);
        let tip = (ex + ux * 0.95 * stroke_w, ey + uy * 0.95 * stroke_w);
        let half = 2.0 * stroke_w;
        let mut pb = PathBuilder::new();
        pb.move_to(tip.0, tip.1);
        pb.line_to(base_c.0 + nx * half, base_c.1 + ny * half);
        pb.line_to(base_c.0 - nx * half, base_c.1 - ny * half);
        pb.close();
        if let Some(h) = pb.finish() {
            pm.fill_path(
                &h,
                &paint(br.rgba, alpha),
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
        Notation::Algebraic | Notation::None => {}
    }
    if p.border || matches!(p.notation, Notation::Algebraic) {
        draw_border_raster(p, pm);
    }
}

/// Algebraic border labels (chessground positions: files along the
/// bottom, ranks along the left; mirrored when flipped).
fn draw_border_raster(p: &Params, pm: &mut Pixmap) {
    let m = p.margin as f32;
    if m < 6.0 {
        return; // no band to draw in
    }
    let fs = (m / 5.5).clamp(2.0, 16.0); // one bitmap pixel
    let col = [0x33, 0x33, 0x33];
    let top = p.caption_h() as f32 + m;
    for x in 0..p.board.width {
        let letter = p.board.file_letter(x).to_string();
        let cx = p.sx(x) + p.cell() / 2.0 - fs * 1.5;
        let cy = if p.flipped {
            top - 5.0 * fs
        } else {
            top + p.size as f32 + (m - 5.0 * fs) / 2.0
        };
        draw_text(pm, &letter, cx, cy, fs, col);
    }
    for y in 0..p.board.height {
        let rank = (y + 1).to_string();
        let cy = p.sy(y) + p.cell() / 2.0 - 2.5 * fs;
        let cx = if p.flipped {
            m + p.size as f32 + (m - 3.0 * fs) / 2.0
        } else {
            (m - 3.5 * fs) / 2.0
        };
        draw_text(pm, &rank, cx, cy, fs, col);
    }
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
    // names/comment render with the embedded Unicode font (Cyrillic etc.);
    // the bar fits a names row and a smaller comment row below it
    let fs = (h * 0.34).clamp(10.0, 34.0);
    let half = w / 2.0 - fs * 0.6;
    let baseline = h * 0.14 + fs * 0.8;
    if !cap.white.is_empty() || !cap.black.is_empty() {
        let white = crate::text::truncate(&cap.white, half, fs);
        let black = crate::text::truncate(&cap.black, half, fs);
        crate::text::draw(
            pm,
            &white,
            p.margin as f32,
            baseline,
            fs,
            [0xff, 0xff, 0xff],
            255,
        );
        let bw = crate::text::width(&black, fs);
        crate::text::draw(
            pm,
            &black,
            w - p.margin as f32 - bw,
            baseline,
            fs,
            [0xff, 0xff, 0xff],
            255,
        );
    }
    if !cap.comment.is_empty() {
        let cfs = fs * 0.78;
        let comment = crate::text::truncate(&cap.comment, w - fs, cfs);
        let cw = crate::text::width(&comment, cfs);
        crate::text::draw(
            pm,
            &comment,
            (w - cw) / 2.0,
            h * 0.62 + cfs * 0.78,
            cfs,
            [0xbb, 0xb3, 0xa8],
            255,
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
        let fs = (ch as f32 * 0.34).clamp(10.0, 34.0);
        let half = w as f32 / 2.0 - fs * 0.6;
        let baseline = ch as f32 * 0.14;
        if !cap.white.is_empty() {
            let white = crate::text::truncate(&cap.white, half, fs);
            o += &svg_text(&white, p.margin as f32, baseline, fs, None);
        }
        if !cap.black.is_empty() {
            let black = crate::text::truncate(&cap.black, half, fs);
            let bw = crate::text::width(&black, fs);
            o += &svg_text(&black, w as f32 - p.margin as f32 - bw, baseline, fs, None);
        }
        if !cap.comment.is_empty() {
            let cfs = fs * 0.78;
            let comment = crate::text::truncate(&cap.comment, w as f32 - fs, cfs);
            let cw = crate::text::width(&comment, cfs);
            o += &svg_text(
                &comment,
                (w as f32 - cw) / 2.0,
                ch as f32 * 0.62,
                cfs,
                Some("#bbb3a8"),
            );
        }
    }
    if !p.transparent {
        o += &format!(
            r##"<rect width="{w}" height="{h}" fill="{}"/>"##,
            crate::theme::hex(p.theme.light),
            w = p.total_w(),
            h = p.total_h()
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
        let op = if piece.kind.is_ghost() {
            " opacity=\"0.3\""
        } else {
            ""
        };
        // custom theme piece sprites: embed the rasterised sprite
        if let Some(set) = p.theme.pieces {
            let man_idx = u8::from(!white);
            let king_idx = 2 + u8::from(!white);
            let (idx, want_crown) = if piece.kind.is_king() && set.has(king_idx) {
                (king_idx, false)
            } else {
                (man_idx, piece.kind.is_king())
            };
            let side = (c * 0.9).ceil().max(1.0) as u32;
            if let Some(sp) = set.sprite(idx, side) {
                if let Ok(png) = sp.encode_png() {
                    use base64::Engine as _;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
                    o += &format!(r#"<g{op}>"#);
                    o += &format!(
                        r##"<image x="{x}" y="{y}" width="{side}" height="{side}" href="data:image/png;base64,{b64}"/>"##,
                        x = fmt(cx - side as f32 / 2.0),
                        y = fmt(cy - side as f32 / 2.0),
                    );
                    if want_crown {
                        o += &svg_crown(cx, cy, c, r);
                    }
                    o += "</g>";
                    continue;
                }
            }
        }
        let (face, stroke) = if white {
            ("#ffffff", "#000000")
        } else {
            ("#1a1a1a", "#dfdfdf")
        };
        o += &format!(r#"<g{op}>"#);
        let rings: &[f32] = if piece.kind.is_king() {
            &[0.0, -0.07, -0.14]
        } else {
            &[0.0, -0.06]
        };
        // wide_crown wall like the raster path: gradient rings below the face
        let n = rings.len();
        for (i, &ro) in rings.iter().enumerate() {
            let fill = if i + 1 < n {
                let (lo, hi) = if white { (0x7f_u8, 0xff) } else { (0x00, 0x9f) };
                let t = i as u16 * (hi as u16 - lo as u16) / (n - 1) as u16 + lo as u16;
                let t = t as u8;
                format!("#{t:02x}{t:02x}{t:02x}")
            } else {
                face.to_string()
            };
            o += &format!(
                r##"<circle cx="{}" cy="{}" r="{}" fill="{fill}" stroke="{stroke}" stroke-width="{}"/>"##,
                fmt(cx),
                fmt(cy + ro * c),
                fmt(r),
                fmt(c * 0.012)
            );
        }
        if piece.kind.is_king() {
            if white {
                o += &format!(
                    r##"<circle cx="{}" cy="{}" r="{}" fill="{face}" stroke="{stroke}" stroke-width="{}"/>"##,
                    fmt(cx),
                    fmt(cy - 0.14 * c),
                    fmt(r),
                    fmt(c * 0.012)
                );
            }
            o += &svg_crown(cx, cy, c, r);
        }
        o += "</g>";
    }
    // arrows, ported from draughtsground svg.ts (markers + round-cap lines)
    if !p.arrows.is_empty() {
        o += "<defs>";
        for a in &p.arrows {
            let br = brush(a.color);
            o += &format!(
                r##"<marker id="arrowhead-{key}" orient="auto" markerWidth="4" markerHeight="8" refX="2.05" refY="2.01"><path d="M0,0 V4 L3,2 Z" fill="{color}"/></marker>"##,
                key = br.key,
                color = br.hex(),
            );
        }
        o += "</defs>";
    }
    for a in &p.arrows {
        let br = brush(a.color);
        let Some((ax, ay)) = p.board.xy(a.from) else {
            continue;
        };
        let Some((bx, by)) = p.board.xy(a.to) else {
            continue;
        };
        let (fx, fy) = p.center(ax, ay);
        let (tx, ty) = p.center(bx, by);
        if a.from == a.to {
            let w = 4.0 * c / 51.2;
            o += &format!(
                r##"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{color}" stroke-width="{w}" opacity="{op}"/>"##,
                fmt(fx),
                fmt(fy),
                fmt(c / 2.0 - w / 2.0),
                color = br.hex(),
                op = fmt(br.opacity),
            );
        } else {
            let stroke_w = br.line_units * c / 51.2;
            let (dx, dy) = (tx - fx, ty - fy);
            let len = (dx * dx + dy * dy).sqrt();
            let m = p.board.width as f32 * p.board.width as f32 * c / 512.0;
            let (ux, uy) = (dx / len, dy / len);
            o += &format!(
                r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{color}" stroke-width="{w}" stroke-linecap="round" marker-end="url(#arrowhead-{key})" opacity="{op}"/>"##,
                fmt(fx),
                fmt(fy),
                fmt(tx - ux * m),
                fmt(ty - uy * m),
                color = br.hex(),
                w = fmt(stroke_w),
                key = br.key,
                op = fmt(br.opacity),
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
                    let letter = p.board.file_letter(x);
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
        Notation::Algebraic | Notation::None => {}
    }
    if p.border || matches!(p.notation, Notation::Algebraic) {
        o += &svg_border(p);
    }
    o += "</svg>";
    o
}

/// Algebraic border labels: files along the bottom, ranks along the
/// left (mirrored when flipped) - chessground positions.
fn svg_border(p: &Params) -> String {
    let m = p.margin as f32;
    if m < 6.0 {
        return String::new();
    }
    let fs = (m / 3.2).clamp(6.0, 22.0); // font size
    let mut o = String::new();
    let top = p.caption_h() as f32 + m;
    for x in 0..p.board.width {
        let letter = p.board.file_letter(x);
        let (tx, ty) = if p.flipped {
            (p.sx(x) + p.cell() / 2.0, top - fs * 0.2)
        } else {
            (p.sx(x) + p.cell() / 2.0, top + p.size as f32 + m * 0.85)
        };
        o += &format!(
            r##"<text x="{x}" y="{y}" font-size="{fs:.0}" fill="#333" text-anchor="middle" font-family="sans-serif">{letter}</text>"##,
            x = fmt(tx),
            y = fmt(ty),
        );
    }
    for y in 0..p.board.height {
        let rank = y + 1;
        let (tx, ty) = if p.flipped {
            (m + p.size as f32 + m * 0.45, p.sy(y) + p.cell() * 0.62)
        } else {
            (m * 0.5, p.sy(y) + p.cell() * 0.62)
        };
        o += &format!(
            r##"<text x="{x}" y="{y}" font-size="{fs:.0}" fill="#333" text-anchor="middle" font-family="sans-serif">{rank}</text>"##,
            x = fmt(tx),
            y = fmt(ty),
        );
    }
    o
}

fn svg_crown(cx: f32, cy: f32, c: f32, r: f32) -> String {
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
    format!(r##"<polygon fill="#bf8c16" points="{pts}"/>"##)
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
