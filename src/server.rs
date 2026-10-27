// HTTP server (std-only): GET /board.png|webp|svg, POST /game.gif,
// GET /healthz, GET /metrics (SPEC.md §2, §6).
// v1: Connection: close, per-connection thread — enough for the render
// workload; swap for axum later if profiling says so.

use crate::gif::{Frame, GifParams};
use crate::metrics::Metrics;
use crate::render::{png, svg, Arrow, Caption, Notation, Params};
use crate::{board, fen, theme};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Instant;

pub struct Server {
    pub metrics: Arc<Metrics>,
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
    if s.len() < 4 || !s.len().is_multiple_of(2) || s.len() > 6 {
        return None;
    }
    let half = s.len() / 2;
    let a: u16 = s[..half].parse().ok()?;
    let b: u16 = s[half..].parse().ok()?;
    (a >= 1 && b >= 1 && a <= max && b <= max).then_some((a, b))
}

pub fn run(bind: &str, metrics: Arc<Metrics>) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind)?;
    eprintln!("boardimage serving on http://{bind}");
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let m = metrics.clone();
        std::thread::spawn(move || {
            let srv = Server { metrics: m };
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
            b'%' if i + 2 < b.len() + 1 && i + 2 < b.len() + 1 => {
                let h = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16);
                match h {
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
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let l = line.trim();
            if l.is_empty() {
                break;
            }
            if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = v.trim().parse().unwrap_or(0);
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

        let started = Instant::now();
        let (status, ctype, body): (&str, &str, Vec<u8>) = match (method.as_str(), path) {
            ("GET", "/healthz") => ("200 OK", "text/plain; charset=utf-8", b"ok".to_vec()),
            ("GET", "/metrics") => (
                "200 OK",
                "text/plain; version=0.0.4; charset=utf-8",
                self.metrics.render().into_bytes(),
            ),
            ("GET", p) if p.starts_with("/board.") => self.board(p, &q),
            ("POST", "/game.gif") => {
                let mut body = vec![0u8; content_length.min(4 << 20)];
                reader.read_exact(&mut body)?;
                self.game_gif(&body)
            }
            _ => (
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"not found".to_vec(),
            ),
        };
        let ok = status.starts_with("200");
        if !ok && path != "/healthz" {
            self.metrics.bad_request();
        }
        let _ = started;
        let payload = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: public, max-age=86400, immutable\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(payload.as_bytes())?;
        stream.write_all(&body)?;
        Ok(())
    }

    fn resolve_board(&self, q: &HashMap<String, String>) -> Result<board::BoardDef, &'static str> {
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
                    .or_else(|| q.get("boardsize"))
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
        if def.width != def.height && q.get("boardHeight").is_none() && q.get("gameType").is_none()
        {
            // non-square only via boardHeight/gameType
            def.height = def.width;
        }
        Ok(def)
    }

    fn board(
        &self,
        path: &str,
        q: &HashMap<String, String>,
    ) -> (&'static str, &'static str, Vec<u8>) {
        let started = Instant::now();
        let format = match path {
            "/board.png" => 0,
            "/board.webp" => 1,
            "/board.svg" => 2,
            _ => unreachable!(),
        };
        let result = self.render_board(path, q);
        match result {
            Ok((ctype, body)) => {
                self.metrics.request(format, false);
                self.metrics.observe_render(started);
                ("200 OK", ctype, body)
            }
            Err(msg) => (
                "400 Bad Request",
                "text/plain; charset=utf-8",
                msg.as_bytes().to_vec(),
            ),
        }
    }

    fn render_board(
        &self,
        path: &str,
        q: &HashMap<String, String>,
    ) -> Result<(&'static str, Vec<u8>), String> {
        let bdef = self.resolve_board(q).map_err(|e| e.to_string())?;
        let fen_s = q.get("fen").ok_or("fen required")?;
        let pos = fen::parse(fen_s, bdef.max_square()).map_err(|e| e.to_string())?;
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
                .and_then(|s| parse_last_move(s, bdef.max_square())),
            squares: q
                .get("squares")
                .map(|s| parse_squares(s, bdef.max_square()))
                .unwrap_or_default(),
            arrows: q
                .get("arrows")
                .map(|s| parse_arrows(s, bdef.max_square()))
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
            "/board.webp" => Err("webp encoder pending: use /board.png (SPEC stage 4)".into()),
            _ => unreachable!(),
        }
    }

    fn game_gif(&self, body: &[u8]) -> (&'static str, &'static str, Vec<u8>) {
        let started = Instant::now();
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
                    (
                        board::BoardDef {
                            width: if (2..=16).contains(&w) && w.is_multiple_of(2) {
                                w
                            } else {
                                10
                            },
                            height: if (2..=16).contains(&h) && h.is_multiple_of(2) {
                                h
                            } else {
                                10
                            },
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
                self.metrics.request(3, false);
                self.metrics.observe_render(started);
                ("200 OK", "image/gif", data)
            }
            Err(e) => (
                "400 Bad Request",
                "text/plain; charset=utf-8",
                e.into_bytes(),
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
            let half = rest.len() / 2;
            if rest.len() < 4 || rest.len() > 6 || rest.len() % 2 != 0 {
                return None;
            }
            let a: u16 = rest[..half].parse().ok()?;
            let b: u16 = rest[half..].parse().ok()?;
            (a >= 1 && b >= 1 && a <= max && b <= max).then_some(Arrow {
                from: a,
                to: b,
                color,
            })
        })
        .collect()
}
