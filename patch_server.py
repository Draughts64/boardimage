# -*- coding: utf-8 -*-
import io

p='src/server.rs'; c=io.open(p,encoding='utf-8').read()

c=c.replace("use crate::gif::{Frame, GifParams};",
"use crate::cache::{sha256_hex, Cache};\nuse crate::gif::{Frame, GifParams};")

c=c.replace("""pub struct Server {
    pub metrics: Arc<Metrics>,
}""","""pub struct Server {
    pub metrics: Arc<Metrics>,
    pub cache: Arc<Cache>,
}

pub fn run(bind: &str, metrics: Arc<Metrics>, cache: Arc<Cache>) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind)?;
    eprintln!("boardimage serving on http://{bind} (cache: {} entries)", cache.len());
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let m = metrics.clone();
        let c = cache.clone();
        std::thread::spawn(move || {
            let srv = Server { metrics: m, cache: c };
            let _ = srv.handle(stream);
        });
    }
    Ok(())
}""")

old_run = '''pub fn run(bind: &str, metrics: Arc<Metrics>) -> std::io::Result<()> {
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
}'''
assert old_run in c, "old run"
c=c.replace(old_run,"",1)

c=c.replace(
 '("GET", "/metrics") => ("200 OK", "text/plain; version=0.0.4; charset=utf-8", self.metrics.render().into_bytes()),',
 '("GET", "/metrics") => ("200 OK", "text/plain; version=0.0.4; charset=utf-8", self.metrics.render().into_bytes(), None),\n'
 '            ("GET", "/openapi.json") => ("200 OK", "application/json", crate::openapi::json().into_bytes(), None),\n'
 '            ("GET", "/swagger") => ("200 OK", "text/html; charset=utf-8", crate::openapi::swagger_html().as_bytes().to_vec(), None),\n'
 '            ("GET", "/redoc") => ("200 OK", "text/html; charset=utf-8", crate::openapi::redoc_html().as_bytes().to_vec(), None),')
c=c.replace(
 '("GET", "/healthz") => ("200 OK", "text/plain; charset=utf-8", b"ok".to_vec()),',
 '("GET", "/healthz") => ("200 OK", "text/plain; charset=utf-8", b"ok".to_vec(), None),')
c=c.replace(
 'let (status, ctype, body): (&str, &str, Vec<u8>) = match (method.as_str(), path) {',
 'let (status, ctype, body, etag_out): (&str, &str, Vec<u8>, Option<String>) = match (method.as_str(), path) {')
c=c.replace(
 '("GET", p) if p.starts_with("/board.") => self.board(p, &q),',
 '("GET", p) if p.starts_with("/board.") => self.board(p, &q, if_none_match.as_deref()),')
c=c.replace(
 '''                reader.read_exact(&mut body)?;
                self.game_gif(&body)
            }
            _ => ("404 Not Found", "text/plain; charset=utf-8", b"not found".to_vec()),''',
 '''                reader.read_exact(&mut body)?;
                self.game_gif(&body, if_none_match.as_deref())
            }
            _ => ("404 Not Found", "text/plain; charset=utf-8", b"not found".to_vec(), None),''')

# headers: capture if-none-match
c=c.replace(
 '''        let mut content_length = 0usize;
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
        }''',
 '''        let mut content_length = 0usize;
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
        }''')

# board(): cache-aware, 4-tuple
old_board = '''    fn board(&self, path: &str, q: &HashMap<String, String>) -> (&'static str, &'static str, Vec<u8>) {
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
            Err(msg) => ("400 Bad Request", "text/plain; charset=utf-8", msg.as_bytes().to_vec()),
        }
    }'''
new_board = '''    fn board(&self, path: &str, q: &HashMap<String, String>, etag_req: Option<&str>) -> (&'static str, &'static str, Vec<u8>, Option<String>) {
        let started = Instant::now();
        let format = match path {
            "/board.png" => 0,
            "/board.webp" => 1,
            "/board.svg" => 2,
            _ => unreachable!(),
        };
        let mut pairs: Vec<(&String, &String)> = q.iter().collect();
        pairs.sort();
        let canonical = format!("{path}?{}", pairs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&"));
        let key = sha256_hex(canonical.as_bytes());
        let ctype = match path {
            "/board.svg" => "image/svg+xml",
            "/board.webp" => "image/webp",
            _ => "image/png",
        };
        if let Some(body) = self.cache.get(&key) {
            self.metrics.request(format, true);
            if etag_req == Some(key.as_str()) {
                return ("304 Not Modified", "text/plain", vec![], Some(key));
            }
            return ("200 OK", ctype, body, Some(key));
        }
        let result = self.render_board(path, q);
        match result {
            Ok((real_ctype, body)) => {
                self.cache.put(&key, body.clone());
                self.metrics.request(format, false);
                self.metrics.observe_render(started);
                self.metrics.set_cache_entries(self.cache.len() as u64);
                ("200 OK", real_ctype, body, Some(key))
            }
            Err(msg) => ("400 Bad Request", "text/plain; charset=utf-8", msg.as_bytes().to_vec(), None),
        }
    }'''
assert old_board in c, "board fn"
c=c.replace(old_board,new_board,1)

# webp route
c=c.replace(
 '"/board.webp" => Err("webp encoder pending: use /board.png (SPEC stage 4)".into()),',
 '''"/board.webp" => {
                let quality: u8 = q.get("quality").and_then(|s| s.parse().ok()).unwrap_or(85).clamp(1, 100);
                Ok(("image/webp", crate::render::webp(&params, quality)?))
            }''')

# game_gif: cache + 4-tuple
c=c.replace(
 '''    fn game_gif(&self, body: &[u8]) -> (&'static str, &'static str, Vec<u8>) {
        let started = Instant::now();''',
 '''    fn game_gif(&self, body: &[u8], etag_req: Option<&str>) -> (&'static str, &'static str, Vec<u8>, Option<String>) {
        let started = Instant::now();
        let key = sha256_hex(body);
        if let Some(data) = self.cache.get(&key) {
            self.metrics.request(3, true);
            if etag_req == Some(key.as_str()) {
                return ("304 Not Modified", "text/plain", vec![], Some(key));
            }
            return ("200 OK", "image/gif", data, Some(key));
        }''')
c=c.replace(
 '''        match go() {
            Ok(data) => {
                self.metrics.request(3, false);
                self.metrics.observe_render(started);
                ("200 OK", "image/gif", data)
            }
            Err(e) => ("400 Bad Request", "text/plain; charset=utf-8", e.into_bytes()),
        }''',
 '''        match go() {
            Ok(data) => {
                self.cache.put(&key, data.clone());
                self.metrics.request(3, false);
                self.metrics.observe_render(started);
                self.metrics.set_cache_entries(self.cache.len() as u64);
                ("200 OK", "image/gif", data, Some(key))
            }
            Err(e) => ("400 Bad Request", "text/plain; charset=utf-8", e.into_bytes(), None),
        }''')

# response: etag header
c=c.replace(
 '''        let payload = format!(
            "HTTP/1.1 {status}\\r\\nContent-Type: {ctype}\\r\\nContent-Length: {}\\r\\nAccess-Control-Allow-Origin: *\\r\\nCache-Control: public, max-age=86400, immutable\\r\\nConnection: close\\r\\n\\r\\n",
            body.len()
        );''',
 '''        let etag_hdr = etag_out.map(|e| format!("ETag: \\"{e}\\"\\r\\n")).unwrap_or_default();
        let payload = format!(
            "HTTP/1.1 {status}\\r\\nContent-Type: {ctype}\\r\\nContent-Length: {}\\r\\nAccess-Control-Allow-Origin: *\\r\\nCache-Control: public, max-age=86400, immutable\\r\\n{etag_hdr}Connection: close\\r\\n\\r\\n",
            body.len()
        );''')

io.open(p,'w',encoding='utf-8',newline='\n').write(c)
print('server.rs wired')

# main.rs
p='src/main.rs'; c=io.open(p,encoding='utf-8').read()
c=c.replace("use boardimage::{cli, metrics::Metrics, server, VERSION};",
"use boardimage::{cache::Cache, cli, metrics::Metrics, server, VERSION};")
c=c.replace('''            let m = Arc::new(Metrics::new());''',
'''            let mem_mb: usize = std::env::var("CACHE_MB").ok().and_then(|v| v.parse().ok()).unwrap_or(512);
            let disk = std::env::var("CACHE_DIR").ok().filter(|d| !d.is_empty());
            let m = Arc::new(Metrics::new());
            let cache = Arc::new(Cache::new(mem_mb, disk, 2048));''')
c=c.replace("if let Err(e) = server::run(&bind, m) {","if let Err(e) = server::run(&bind, m, cache) {")
io.open(p,'w',encoding='utf-8',newline='\n').write(c)
print('main.rs wired')

# lib.rs
p='src/lib.rs'; c=io.open(p,encoding='utf-8').read()
c=c.replace("pub mod server;","pub mod cache;\npub mod openapi;\npub mod server;")
io.open(p,'w',encoding='utf-8',newline='\n').write(c)
print('lib.rs wired')

# render.rs: webp
p='src/render.rs'; c=io.open(p,encoding='utf-8').read()
if 'pub fn webp(' not in c:
    c += '''
pub fn webp(p: &Params, quality: u8) -> Result<Vec<u8>, String> {
    let pm = raster(p);
    let img = image::RgbaImage::from_raw(pm.width(), pm.height(), pm.data().to_vec())
        .ok_or("pixmap conversion")?;
    let (w, h) = img.dimensions();
    let encoder = webp::Encoder::from_rgba(img.into_raw().as_slice(), w, h);
    if quality >= 100 {
        Ok(encoder.encode_lossless().to_vec())
    } else {
        Ok(encoder.encode(f32::from(quality)).to_vec())
    }
}
'''
    io.open(p,'w',encoding='utf-8',newline='\n').write(c)
    print('render.rs webp added')
