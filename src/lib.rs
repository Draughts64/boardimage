// boardimage — draughts board renderer (SPEC.md).
// Library core: FEN parsing, board geometry, SVG/raster rendering,
// PNG/GIF encoding, Prometheus metrics, HTTP server and CLI.

pub mod board;
pub mod cache;
pub mod chess;
pub mod cli;
pub mod fen;
pub mod gif;
pub mod glyph;
pub mod metrics;
pub mod openapi;
pub mod render;
pub mod server;
pub mod text;
pub mod theme;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
