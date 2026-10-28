// Unicode text for captions: embedded DejaVu Sans (Latin, Cyrillic,
// Greek and more; see assets/DejaVuSans.LICENSE). Glyph outlines are
// converted to tiny-skia paths and filled — crisp vectors, no raster
// cache, works in PNG/WebP and every GIF frame. Square numbers and
// border labels stay on the 3x5 bitmap font (ASCII by nature).

use ab_glyph::{Font as _, FontArc, Point, PxScale, ScaleFont as _};
use std::sync::OnceLock;
use tiny_skia::Pixmap;

static FONT: OnceLock<FontArc> = OnceLock::new();

fn font() -> &'static FontArc {
    FONT.get_or_init(|| {
        FontArc::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf"))
            .expect("embedded DejaVuSans.ttf is valid")
    })
}

/// Render `text` with `baseline_y` as the baseline (top-left origin x).
/// Anti-aliased per-pixel coverage from the font outline.
pub fn draw(
    pm: &mut Pixmap,
    text: &str,
    x: f32,
    baseline_y: f32,
    size: f32,
    color: [u8; 3],
    alpha: u8,
) {
    let font = font();
    let scale = PxScale::from(size);
    let scaled = font.as_scaled(scale);
    let (w, h) = (pm.width(), pm.height());
    let mut cx = x;
    let mut any = false;
    for ch in text.chars() {
        let gid = scaled.glyph_id(ch);
        if !ch.is_whitespace() {
            let g = gid.with_scale_and_position(
                scale,
                Point {
                    x: cx,
                    y: baseline_y,
                },
            );
            if let Some(outlined) = font.outline_glyph(g) {
                any = true;
                // draw() yields coords RELATIVE to the glyph's bounding
                // box - offset them onto the canvas (ab_glyph docs)
                let b = outlined.px_bounds();
                let (ox, oy) = (b.min.x.floor() as i32, b.min.y.floor() as i32);
                outlined.draw(|gx, gy, cov| {
                    let gx = ox + gx as i32;
                    let gy = oy + gy as i32;
                    if gx < 0 || gy < 0 || gx >= w as i32 || gy >= h as i32 || cov <= 0.0 {
                        return;
                    }
                    // blend into the premultiplied RGBA8 bytes directly
                    let a = (cov * alpha as f32).min(1.0) * 255.0;
                    let data = pm.data_mut();
                    let o = ((gy as u32 * w + gx as u32) * 4) as usize;
                    let blend = |d: f32, c: u8| {
                        let s = c as f32 * a / 255.0; // premultiplied source
                        let out = s + d * (1.0 - a / 255.0);
                        out.min(a.max(0.0)) as u8 // premultiplied: channel <= alpha
                    };
                    data[o] = blend(data[o] as f32, color[0]);
                    data[o + 1] = blend(data[o + 1] as f32, color[1]);
                    data[o + 2] = blend(data[o + 2] as f32, color[2]);
                    let da = data[o + 3] as f32;
                    data[o + 3] = (a + da * (1.0 - a / 255.0)).min(255.0) as u8;
                });
            }
        }
        cx += scaled.h_advance(gid);
    }
    let _ = any;
}

/// Advance width of `text` at `size` px.
pub fn width(text: &str, size: f32) -> f32 {
    let font = font();
    let scaled = font.as_scaled(PxScale::from(size));
    text.chars()
        .map(|ch| scaled.h_advance(scaled.glyph_id(ch)))
        .sum()
}

/// Cut the string with an ellipsis so its width fits max_px.
pub fn truncate(s: &str, max_px: f32, size: f32) -> String {
    if width(s, size) <= max_px {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        let t = format!("{out}{ch}…");
        if width(&t, size) > max_px {
            return format!("{out}…");
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_scales_and_covers_cyrillic() {
        assert!(width("hello", 16.0) > 20.0);
        assert!(width("Иванов", 16.0) > 30.0, "cyrillic has advances");
        assert!(width("Иванов", 32.0) > width("Иванов", 16.0));
        assert_eq!(width("", 16.0), 0.0);
    }

    #[test]
    fn draw_cyrillic_produces_pixels() {
        let mut pm = Pixmap::new(220, 50).unwrap();
        draw(
            &mut pm,
            "Иванов Александр",
            4.0,
            34.0,
            22.0,
            [255, 255, 255],
            255,
        );
        let hits = pm
            .pixels()
            .iter()
            .filter(|p| p.red() > 200 && p.green() > 200)
            .count();
        assert!(hits > 60, "cyrillic glyphs rasterised: {hits} px");
    }

    #[test]
    fn truncate_ellipsis() {
        let s = "Иванов Александр Петрович";
        let t = truncate(s, 80.0, 16.0);
        assert!(t.ends_with('…'));
        assert!(width(&t, 16.0) <= 80.0 + width("…", 16.0));
        assert_eq!(truncate("abc", 500.0, 16.0), "abc");
    }
}
