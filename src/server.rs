// HTTP server (std-only): board images with ETag/304 caching,
// GIF replays, health, Prometheus metrics, OpenAPI/Swagger (SPEC.md §2, §5, §6).
// v1: Connection: close, thread-per-connection — fine for the render
// workload; swap for axum if profiling ever says so.

use crate::cache::{sha256_hex, Cache};
use crate::gif::{Frame, GifParams};
use crate::metrics::Metrics;
use crate::render::{png, svg, webp, Arrow, Caption, Notation, Params};
use crate::{board, fen, theme};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Instant;

pub struct Server {
    pub metrics: Arc<Metrics>,
    pub cache: Arc<Cache>,
}

#[derive(Deserialize)]
struct GifRequest {
    #[serde(default)]
    board_size: Option<u16>,
    board_height: Option<u16>,
    #[serde(default)]
    light_squares: bool,
    #[serde(default)]
    numbering: u8,
    game_type: Option<u32>,
    variant: Option<String>,
    #[serde(default = "d_size")]
    size: u32,
    theme: Option<String>,
    #[serde(default)]
    flat: bool,
    #[serde(default)]
    orientation: Option<String>,
    #[serde(default = "d_delay")]
    delay_ms: u32,
    #[serde(default)]
    white: String,
    #[serde(default)]
    black: String,
    frames: Vec<GifFrame>,
}

#[derive(Deserialize)]
struct GifFrame {
    fen: String,
    last_move: Option<String>,
}

fn d_size() -> u32 {
    360
}
fn d_delay() -> u32 {
    700
}

fn parse_last_move(s: &str, max: u16) -> Option<(u16, u16)> {
    let l = s.len();
    if !(4..=6).contains(&l) || !l.is_multiple_of(2) {
        return None;
    }
    let a: u16 = s[..l / 2].parse().ok()?;
    let b: u16 = s[l / 2..].parse().ok()?;
    if (1..=max).contains(&a) && (1..=max).contains(&b) {
        Some((a, b))
    } else {
        None
    }
}

pub fn run(bind: &str, metrics: Arc<Metrics>, cache: Arc<Cache>) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind)?;
    eprintln!(
        "boardimage serving on http://{bind} (cache: {} entries)",
        cache.len()
    );
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let m = metrics.clone();
        let c = cache.clone();
        std::thread::spawn(move || {
            let srv = Server {
                metrics: m,
                cache: c,
            };
            let _ = srv.handle(stream);
        });
    }
    Ok(())
}

fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut o = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(v) => {
                        o.push(v);
                        i += 3;
                    }
                    Err(_) => {
                        o.push(b[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                o.push(b' ');
                i += 1;
            }
            c => {
                o.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&o).into_owned()
}

impl Server {
    fn handle(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut first = String::new();
        reader.read_line(&mut first)?;
        let mut parts = first.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let target = parts.next().unwrap_or("/").to_string();
        let mut content_length = 0usize;
        let mut if_none_match: Option<String> = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let l = line.trim();
            if l.is_empty() {
                break;
            }
            let low = l.to_ascii_lowercase();
            if let Some(v) = low.strip_prefix("content-length:") {
                content_length = v.trim().parse().unwrap_or(0);
            }
            if let Some(v) = low.strip_prefix("if-none-match:") {
                if_none_match = Some(v.trim().trim_matches('"').to_string());
            }
        }
        let (path, query) = match target.split_once('?') {
            Some((p, q)) => (p, q),
            None => (target.as_str(), ""),
        };
        let q: HashMap<String, String> = query
            .split('&')
            .filter_map(|kv| {
                kv.split_once('=')
                    .map(|(k, v)| (k.to_string(), urldecode(v)))
            })
            .collect();

        let (status, ctype, body, etag_out): (&str, &str, Vec<u8>, Option<String>) =
            match (method.as_str(), path) {
                ("GET", "/healthz") => {
                    ("200 OK", "text/plain; charset=utf-8", b"ok".to_vec(), None)
                }
                ("GET", "/metrics") => (
                    "200 OK",
                    "text/plain; version=0.0.4; charset=utf-8",
                    self.metrics.render().into_bytes(),
                    None,
                ),
                ("GET", "/openapi.json") => (
                    "200 OK",
                    "application/json",
                    crate::openapi::json().into_bytes(),
                    None,
                ),
                ("GET", "/swagger") => (
                    "200 OK",
                    "text/html; charset=utf-8",
                    crate::openapi::swagger_html().as_bytes().to_vec(),
                    None,
                ),
                ("GET", "/redoc") => (
                    "200 OK",
                    "text/html; charset=utf-8",
                    crate::openapi::redoc_html().as_bytes().to_vec(),
                    None,
                ),
                ("GET", p) if p.starts_with("/board.") => {
                    self.board(p, &q, if_none_match.as_deref())
                }
                ("POST", "/game.gif") => {
                    let mut body = vec![0u8; content_length.min(4 << 20)];
                    reader.read_exact(&mut body)?;
                    self.game_gif(&body, if_none_match.as_deref())
                }
                _ => (
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    b"not found".to_vec(),
                    None,
                ),
            };
        if !status.starts_with("200") && !status.starts_with("304") && path != "/healthz" {
            self.metrics.bad_request();
        }
        let etag_hdr = etag_out
            .map(|e| format!("ETag: \"{e}\"\r\n"))
            .unwrap_or_default();
        let payload = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: public, max-age=86400, immutable\r\n{etag_hdr}Connection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(payload.as_bytes())?;
        stream.write_all(&body)?;
        Ok(())
    }

    fn board(
        &self,
        path: &str,
        q: &HashMap<String, String>,
        etag_req: Option<&str>,
    ) -> (&'static str, &'static str, Vec<u8>, Option<String>) {
        let started = Instant::now();
        let format = match path {
            "/board.png" => 0,
            "/board.webp" => 1,
            "/board.svg" => 2,
            _ => unreachable!(),
        };
        let mut pairs: Vec<(&String, &String)> = q.iter().collect();
        pairs.sort();
        let canonical = format!(
            "{path}?{}",
            pairs
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&")
        );
        let key = sha256_hex(canonical.as_bytes());
        let cached_ctype = match path {
            "/board.svg" => "image/svg+xml",
            "/board.webp" => "image/webp",
            _ => "image/png",
        };
        if let Some(body) = self.cache.get(&key) {
            self.metrics.request(format, true);
            if etag_req == Some(key.as_str()) {
                return ("304 Not Modified", "text/plain", vec![], Some(key));
            }
            return ("200 OK", cached_ctype, body, Some(key));
        }
        let result = self.render_board(path, q);
        match result {
            Ok((ctype, body)) => {
                self.cache.put(&key, body.clone());
                self.metrics.request(format, false);
                self.metrics.observe_render(started);
                self.metrics.set_cache_entries(self.cache.len() as u64);
                ("200 OK", ctype, body, Some(key))
            }
            Err(msg) => (
                "400 Bad Request",
                "text/plain; charset=utf-8",
                msg.as_bytes().to_vec(),
                None,
            ),
        }
    }

    fn render_board(
        &self,
        path: &str,
        q: &HashMap<String, String>,
    ) -> Result<(&'static str, Vec<u8>), String> {
        let bdef = self.resolve_board(q)?;
        let max = bdef.max_square();
        let fen_s = q.get("fen").ok_or("fen required")?;
        let pos = fen::parse(fen_s, max).map_err(|e| e.to_string())?;
        let size: u32 = q
            .get("size")
            .and_then(|s| s.parse().ok())
            .unwrap_or(360)
            .clamp(16, 1024);
        let th = q
            .get("theme")
            .and_then(|t| theme::by_name(t))
            .unwrap_or_else(theme::default_theme);
        let params = Params {
            pos,
            board: bdef,
            size,
            theme: th,
            flat: q.get("pieceStyle").is_some_and(|s| s == "flat")
                || q.get("piece").is_some_and(|s| s == "flat"),
            flipped: q.get("orientation").is_some_and(|s| s == "black"),
            last_move: q
                .get("lastMove")
                .or(q.get("lastmove"))
                .and_then(|s| parse_last_move(s, max)),
            squares: q
                .get("squares")
                .map(|s| parse_squares(s, max))
                .unwrap_or_default(),
            arrows: q
                .get("arrows")
                .map(|s| parse_arrows(s, max))
                .unwrap_or_default(),
            notation: match q.get("notation").map(|s| s.as_str()) {
                Some("numeric") => Notation::Numeric,
                Some("algebraic") => Notation::Algebraic,
                Some("coords") => Notation::Coords,
                _ => Notation::None,
            },
            margin: q
                .get("margin")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
                .min(64),
            transparent: q
                .get("transparent")
                .is_some_and(|s| s == "true" || s == "1"),
            caption: Some(Caption {
                white: q.get("white").cloned().unwrap_or_default(),
                black: q.get("black").cloned().unwrap_or_default(),
                comment: q.get("comment").cloned().unwrap_or_default(),
            })
            .filter(|c| !c.white.is_empty() || !c.black.is_empty() || !c.comment.is_empty()),
        };
        match path {
            "/board.svg" => Ok(("image/svg+xml", svg(&params).into_bytes())),
            "/board.png" => Ok(("image/png", png(&params)?)),
            "/board.webp" => {
                let quality: u8 = q
                    .get("quality")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(85)
                    .clamp(1, 100);
                Ok(("image/webp", webp(&params, quality)?))
            }
            _ => unreachable!(),
        }
    }

    fn resolve_board(&self, q: &HashMap<String, String>) -> Result<board::BoardDef, String> {
        let resolved = q
            .get("gameType")
            .and_then(|g| g.parse().ok())
            .and_then(board::from_game_type)
            .or_else(|| q.get("variant").and_then(|v| board::from_variant(v)));
        let mut def = match resolved {
            Some((d, _)) => d,
            None => board::BoardDef {
                width: q
                    .get("boardSize")
                    .or(q.get("boardsize"))
                    .and_then(|s| s.parse().ok())
                    .filter(|v| *v >= 2 && *v <= 16 && v % 2 == 0)
                    .unwrap_or(10),
                height: q
                    .get("boardHeight")
                    .and_then(|s| s.parse().ok())
                    .filter(|v| *v >= 2 && *v <= 16 && v % 2 == 0)
                    .unwrap_or(10),
                light_squares: false,
                numbering: 0,
            },
        };
        if let Some(ls) = q.get("lightSquares") {
            def.light_squares = ls == "true" || ls == "1";
        }
        if let Some(n) = q.get("numbering").and_then(|s| s.parse().ok()) {
            if n <= 3 {
                def.numbering = n;
            }
        }
        Ok(def)
    }

    fn game_gif(
        &self,
        body: &[u8],
        etag_req: Option<&str>,
    ) -> (&'static str, &'static str, Vec<u8>, Option<String>) {
        let started = Instant::now();
        let key = sha256_hex(body);
        if let Some(data) = self.cache.get(&key) {
            self.metrics.request(3, true);
            if etag_req == Some(key.as_str()) {
                return ("304 Not Modified", "text/plain", vec![], Some(key));
            }
            return ("200 OK", "image/gif", data, Some(key));
        }
        let go = || -> Result<Vec<u8>, String> {
            let req: GifRequest = serde_json::from_slice(body).map_err(|e| e.to_string())?;
            if req.frames.is_empty() || req.frames.len() > 500 {
                return Err("frames: 1..500 required".into());
            }
            let size = req.size.clamp(16, 640);
            let (bdef, _) = req
                .game_type
                .and_then(board::from_game_type)
                .or_else(|| req.variant.as_deref().and_then(board::from_variant))
                .unwrap_or_else(|| {
                    let w = req.board_size.unwrap_or(10);
                    let h = req.board_height.unwrap_or(w);
                    let cw = if (2..=16).contains(&w) && w.is_multiple_of(2) {
                        w
                    } else {
                        10
                    };
                    let ch = if (2..=16).contains(&h) && h.is_multiple_of(2) {
                        h
                    } else {
                        10
                    };
                    (
                        board::BoardDef {
                            width: cw,
                            height: ch,
                            light_squares: req.light_squares,
                            numbering: req.numbering.min(3),
                        },
                        "standard",
                    )
                });
            let th = req
                .theme
                .as_deref()
                .and_then(theme::by_name)
                .unwrap_or_else(theme::default_theme);
            let frames: Vec<Frame> = req
                .frames
                .into_iter()
                .map(|f| Frame {
                    last_move: f
                        .last_move
                        .and_then(|s| parse_last_move(&s, bdef.max_square())),
                    fen: f.fen,
                })
                .collect();
            let gparams = GifParams {
                board: bdef,
                size,
                theme: th,
                flat: req.flat,
                flipped: req.orientation.as_deref() == Some("black"),
                delay_ms: req.delay_ms,
                caption: (!req.white.is_empty() || !req.black.is_empty()).then(|| Caption {
                    white: req.white,
                    black: req.black,
                    comment: String::new(),
                }),
            };
            crate::gif::encode(&gparams, &frames)
        };
        match go() {
            Ok(data) => {
                self.cache.put(&key, data.clone());
                self.metrics.request(3, false);
                self.metrics.observe_render(started);
                self.metrics.set_cache_entries(self.cache.len() as u64);
                ("200 OK", "image/gif", data, Some(key))
            }
            Err(e) => (
                "400 Bad Request",
                "text/plain; charset=utf-8",
                e.into_bytes(),
                None,
            ),
        }
    }
}

fn parse_squares(s: &str, max: u16) -> Vec<(u16, f32)> {
    s.split(',')
        .filter_map(|item| {
            let (sq, alpha) = match item.split_once(':') {
                Some((a, b)) => (a, b.parse::<f32>().unwrap_or(0.5)),
                None => (item, 0.5),
            };
            sq.trim()
                .parse::<u16>()
                .ok()
                .filter(|v| (1..=max).contains(v))
                .map(|v| (v, alpha.clamp(0.0, 1.0)))
        })
        .collect()
}

fn parse_arrows(s: &str, max: u16) -> Vec<Arrow> {
    s.split(',')
        .filter(|i| !i.trim().is_empty())
        .filter_map(|item| {
            let (color, rest) = match item.chars().next() {
                Some(c @ ('G' | 'B' | 'R' | 'Y')) => (Some(c), &item[1..]),
                _ => (None, item),
            };
            let l = rest.len();
            if !(4..=6).contains(&l) || l % 2 != 0 {
                return None;
            }
            let a: u16 = rest[..l / 2].parse().ok()?;
            let b: u16 = rest[l / 2..].parse().ok()?;
            if (1..=max).contains(&a) && (1..=max).contains(&b) {
                Some(Arrow {
                    from: a,
                    to: b,
                    color,
                })
            } else {
                None
            }
        })
        .collect()
}
