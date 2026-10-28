// Board themes (SPEC.md §4.4): 6 built-ins plus custom themes loaded
// from a directory (THEMES_DIR), lichess-style: board colours + optional
// piece sprites. Sprites are SVG (rasterised via resvg) or PNG, scaled
// per cell size and cached.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

type SpriteCache = Mutex<HashMap<(u8, u32), Arc<tiny_skia::Pixmap>>>;
type OverrideCache = Mutex<HashMap<([u8; 3], [u8; 3]), &'static Theme>>;

#[derive(Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub light: [u8; 3],
    pub dark: [u8; 3],
    pub last_light: [u8; 3],
    pub last_dark: [u8; 3],
    pub pieces: Option<&'static PieceSet>,
}

pub enum PieceArt {
    Svg(String),
    Png(Vec<u8>),
}

pub struct PieceSet {
    // wm, bm, wk, bk; king slots fall back to the man sprite + crown
    art: [Option<PieceArt>; 4],
    cache: OnceLock<SpriteCache>,
}

impl PieceSet {
    pub fn new(wm: PieceArt, bm: PieceArt, wk: Option<PieceArt>, bk: Option<PieceArt>) -> Self {
        Self {
            art: [Some(wm), Some(bm), wk, bk],
            cache: OnceLock::new(),
        }
    }

    /// idx: 0 = white man, 1 = black man, 2 = white king, 3 = black
    /// king. `size` is the sprite side in pixels.
    pub fn sprite(&self, idx: u8, size: u32) -> Option<Arc<tiny_skia::Pixmap>> {
        if size == 0 || idx > 3 {
            return None;
        }
        let cache = self.cache.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(p) = cache.lock().unwrap().get(&(idx, size)) {
            return Some(p.clone());
        }
        let pm = render_art(self.art[idx as usize].as_ref()?, size)?;
        let pm = Arc::new(pm);
        cache.lock().unwrap().insert((idx, size), pm.clone());
        Some(pm)
    }

    pub fn has(&self, idx: u8) -> bool {
        idx <= 3 && self.art[idx as usize].is_some()
    }
}

fn render_art(art: &PieceArt, size: u32) -> Option<tiny_skia::Pixmap> {
    match art {
        PieceArt::Svg(src) => {
            let tree = resvg::usvg::Tree::from_str(src, &resvg::usvg::Options::default()).ok()?;
            let mut pm = resvg::tiny_skia::Pixmap::new(size, size)?;
            // scale the natural size (or viewBox) to the target side
            let sz = tree.size();
            let t = resvg::tiny_skia::Transform::from_scale(
                size as f32 / sz.width().max(1.0),
                size as f32 / sz.height().max(1.0),
            );
            resvg::render(&tree, t, &mut pm.as_mut());
            let data = pm.data().to_vec();
            tiny_skia::Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(size, size)?)
        }
        PieceArt::Png(bytes) => {
            let img = image::load_from_memory(bytes).ok()?.to_rgba8();
            let img =
                image::imageops::resize(&img, size, size, image::imageops::FilterType::Triangle);
            tiny_skia::Pixmap::from_vec(img.into_raw(), tiny_skia::IntSize::from_wh(size, size)?)
        }
    }
}

impl std::fmt::Debug for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Theme")
            .field("name", &self.name)
            .field("light", &self.light)
            .field("dark", &self.dark)
            .field("pieces", &self.pieces.is_some())
            .finish()
    }
}

const LAST: [u8; 3] = [0xcd, 0xd2, 0x6a];
const LAST_DARK: [u8; 3] = [0xaa, 0xa2, 0x3b];

pub const THEMES: &[Theme] = &[
    Theme {
        name: "default",
        light: [0xf0, 0xd9, 0xb5],
        dark: [0xb5, 0x88, 0x63],
        last_light: LAST,
        last_dark: LAST_DARK,
        pieces: None,
    },
    Theme {
        name: "lidraughts-brown",
        light: [0xff, 0xce, 0x9e],
        dark: [0xd1, 0x8b, 0x47],
        last_light: LAST,
        last_dark: LAST_DARK,
        pieces: None,
    },
    Theme {
        name: "lidraughts-blue",
        light: [0xde, 0xe3, 0xe6],
        dark: [0x8c, 0xa2, 0xad],
        last_light: LAST,
        last_dark: LAST_DARK,
        pieces: None,
    },
    Theme {
        name: "green",
        light: [0xff, 0xff, 0xdd],
        dark: [0x86, 0xa6, 0x66],
        last_light: LAST,
        last_dark: LAST_DARK,
        pieces: None,
    },
    Theme {
        name: "marble",
        light: [0xe8, 0xe8, 0xe8],
        dark: [0x99, 0x99, 0x99],
        last_light: LAST,
        last_dark: LAST_DARK,
        pieces: None,
    },
    Theme {
        name: "wood",
        light: [0xd0, 0xb0, 0x78],
        dark: [0x84, 0x56, 0x33],
        last_light: LAST,
        last_dark: LAST_DARK,
        pieces: None,
    },
];

static CUSTOM: OnceLock<Mutex<Vec<&'static Theme>>> = OnceLock::new();
static OVERRIDES: OnceLock<OverrideCache> = OnceLock::new();
const MAX_OVERRIDES: usize = 1024;

pub fn by_name(name: &str) -> Option<&'static Theme> {
    if let Some(t) = THEMES.iter().find(|t| t.name == name) {
        return Some(t);
    }
    CUSTOM
        .get()
        .and_then(|c| c.lock().unwrap().iter().find(|t| t.name == name).copied())
}

pub fn default_theme() -> &'static Theme {
    by_name("lidraughts-brown").unwrap()
}

/// Inline colour overrides on top of any theme (capped leak cache, so
/// repeated ?light=…&dark=… pairs reuse one allocation).
pub fn with_colors(base: &Theme, light: Option<[u8; 3]>, dark: Option<[u8; 3]>) -> &'static Theme {
    let l = light.unwrap_or(base.light);
    let d = dark.unwrap_or(base.dark);
    if l == base.light && d == base.dark {
        // SAFETY-free path: find the identical static theme
        // (built-ins and registered customs are all &'static)
        let same = THEMES
            .iter()
            .find(|t| t.light == l && t.dark == d && t.pieces.is_none());
        if let Some(t) = same {
            return t;
        }
    }
    let cell = OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut m = cell.lock().unwrap();
    if let Some(t) = m.get(&(l, d)) {
        return t;
    }
    if m.len() >= MAX_OVERRIDES {
        return base_static(base);
    }
    let t: &'static Theme = Box::leak(Box::new(Theme {
        name: "custom",
        light: l,
        dark: d,
        last_light: base.last_light,
        last_dark: base.last_dark,
        pieces: base.pieces,
    }));
    m.insert((l, d), t);
    t
}

fn base_static(base: &Theme) -> &'static Theme {
    THEMES
        .iter()
        .find(|t| t.name == base.name)
        .unwrap_or(&THEMES[0])
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// "#rrggbb" or "rrggbb"
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[derive(serde::Deserialize)]
struct ThemeJson {
    light: String,
    dark: String,
    #[serde(default)]
    last_light: Option<String>,
    #[serde(default)]
    last_dark: Option<String>,
    #[serde(default)]
    pieces: Option<PiecesJson>,
}

#[derive(serde::Deserialize)]
struct PiecesJson {
    #[serde(default)]
    wm: Option<String>,
    #[serde(default)]
    bm: Option<String>,
    #[serde(default)]
    wk: Option<String>,
    #[serde(default)]
    bk: Option<String>,
}

fn load_art(dir: &Path, file: &str) -> Option<PieceArt> {
    let p = dir.join(file);
    match p.extension().and_then(|e| e.to_str()) {
        Some("svg") => std::fs::read_to_string(p).ok().map(PieceArt::Svg),
        Some("png") => std::fs::read(p).ok().map(PieceArt::Png),
        _ => None,
    }
}

/// Load every `<dir>/<name>/theme.json` as a custom theme. Themes that
/// fail to parse are skipped with a warning. Returns the loaded count.
pub fn load_dir(dir: &str) -> usize {
    let root = Path::new(dir);
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    let mut n = 0;
    for e in entries.flatten() {
        let sub = e.path();
        let Some(name) = e.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(manifest) = std::fs::read_to_string(sub.join("theme.json")) else {
            continue;
        };
        let Ok(j) = serde_json::from_str::<ThemeJson>(&manifest) else {
            eprintln!("boardimage: theme {name}: invalid theme.json");
            continue;
        };
        let (Some(light), Some(dark)) = (parse_hex(&j.light), parse_hex(&j.dark)) else {
            eprintln!("boardimage: theme {name}: light/dark must be #rrggbb");
            continue;
        };
        if by_name(&name).is_some() {
            eprintln!("boardimage: theme {name}: shadowing a built-in name, skipped");
            continue;
        }
        let pieces = j.pieces.and_then(|p| {
            let (Some(wm), Some(bm)) = (p.wm, p.bm) else {
                eprintln!("boardimage: theme {name}: pieces need at least wm and bm");
                return None;
            };
            let (wm, bm) = (load_art(&sub, &wm)?, load_art(&sub, &bm)?);
            let wk = p.wk.and_then(|f| load_art(&sub, &f));
            let bk = p.bk.and_then(|f| load_art(&sub, &f));
            Some(&*Box::leak(Box::new(PieceSet::new(wm, bm, wk, bk))))
        });
        let leaked_name: &'static str = Box::leak(name.into_boxed_str());
        let t: &'static Theme = Box::leak(Box::new(Theme {
            name: leaked_name,
            light,
            dark,
            last_light: j.last_light.and_then(|s| parse_hex(&s)).unwrap_or(LAST),
            last_dark: j.last_dark.and_then(|s| parse_hex(&s)).unwrap_or(LAST_DARK),
            pieces,
        }));
        CUSTOM
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .push(t);
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(parse_hex("#ffce9e"), Some([0xff, 0xce, 0x9e]));
        assert_eq!(parse_hex("ffce9e"), Some([0xff, 0xce, 0x9e]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("zzz000"), None);
        assert_eq!(hex([0xff, 0xce, 0x9e]), "#ffce9e");
    }

    #[test]
    fn builtin_lookup() {
        assert!(by_name("wood").is_some());
        assert!(by_name("nope").is_none());
    }

    #[test]
    fn sprite_png_and_fallback() {
        // 8x8 solid red PNG built with the image crate
        let mut img = image::RgbaImage::new(8, 8);
        for px in img.pixels_mut() {
            *px = image::Rgba([255, 0, 0, 255]);
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let set = PieceSet::new(
            PieceArt::Png(bytes.into_inner()),
            PieceArt::Png(vec![]), // broken on purpose
            None,
            None,
        );
        let sp = set.sprite(0, 16).unwrap();
        assert_eq!((sp.width(), sp.height()), (16, 16));
        let px = sp.pixel(8, 8).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (255, 0, 0));
        assert!(set.sprite(1, 16).is_none(), "broken art is None");
        assert!(!set.has(2) && !set.has(3));
        // cached call returns the same allocation
        assert!(std::ptr::eq(
            set.sprite(0, 16).unwrap().as_ref(),
            sp.as_ref()
        ));
    }

    #[test]
    fn sprite_svg() {
        let set = PieceSet::new(
            PieceArt::Svg(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#00ff00"/></svg>"##
                    .into(),
            ),
            PieceArt::Svg(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#0000ff"/></svg>"##
                    .into(),
            ),
            None,
            None,
        );
        let sp = set.sprite(0, 32).unwrap();
        let px = sp.pixel(16, 16).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (0, 255, 0));
    }

    #[test]
    fn with_colors_overrides_and_caches() {
        let base = default_theme();
        let t1 = with_colors(base, Some([1, 2, 3]), Some([4, 5, 6]));
        let t2 = with_colors(base, Some([1, 2, 3]), Some([4, 5, 6]));
        assert_eq!(t1.light, [1, 2, 3]);
        assert!(std::ptr::eq(t1, t2), "same pair reuses the cached theme");
        let t3 = with_colors(base, None, None);
        assert_eq!(t3.light, base.light, "no overrides -> base colours");
    }
}
