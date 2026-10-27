// boardimage — web service + CLI (SPEC.md).
//   boardimage serve [--bind 127.0.0.1:8080]   (default)
//   boardimage render --fen ... --out file.png [png|svg|gif]

use boardimage::{cli, metrics::Metrics, server, VERSION};
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("render") => {
            if let Err(e) = cli::run_render(&args[1..]) {
                eprintln!("render error: {e}");
                std::process::exit(1);
            }
        }
        Some("serve") | None => {
            let bind = args
                .iter()
                .position(|a| a == "--bind")
                .and_then(|i| args.get(i + 1).cloned())
                .or_else(|| std::env::var("BIND").ok())
                .unwrap_or_else(|| "127.0.0.1:8080".into());
            let m = Arc::new(Metrics::new());
            eprintln!("boardimage v{VERSION}");
            if let Err(e) = server::run(&bind, m) {
                eprintln!("server error: {e}");
                std::process::exit(1);
            }
        }
        Some(other) => {
            eprintln!("unknown command: {other}\nusage: boardimage [serve [--bind ADDR] | render --fen FEN --out FILE]");
            std::process::exit(2);
        }
    }
}
