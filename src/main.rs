// boardimage — draughts board rendering service (SPEC.md).
// Stage 0 skeleton: zero-dependency HTTP listener with /healthz,
// so the repo builds and the compose healthcheck has something to talk to.
// Real rendering arrives per SPEC.md §10 (spike first: sprites + gifski).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

fn main() {
    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = TcpListener::bind(&bind).expect("bind");
    eprintln!("boardimage stage-0 listening on http://{bind}");
    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            handle(stream);
        }
    }
}

fn handle(mut stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let (status, body) = match path {
        "/healthz" => ("200 OK", "ok"),
        _ => ("404 Not Found", "not found (stage 0 — see SPEC.md)"),
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}
