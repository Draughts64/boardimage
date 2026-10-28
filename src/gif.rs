// GIF replays via gifski (SPEC.md §2.3). The game engine lives in the
// monolith — frames arrive as (fen, lastMove) pairs.

use crate::board::BoardDef;
use crate::fen;
use crate::render::{raster, Caption, Params};
use crate::theme;
use std::io::Write;
use std::sync::{Arc, Mutex};

pub struct Frame {
    pub fen: String,
    pub last_move: Option<(u16, u16)>,
}

pub struct GifParams {
    pub board: BoardDef,
    pub size: u32,
    pub theme: &'static theme::Theme,
    pub flat: bool,
    pub flipped: bool,
    pub delay_ms: u32,
    pub caption: Option<Caption>,
    pub comment: String,
}

struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn encode(params: &GifParams, frames: &[Frame]) -> Result<Vec<u8>, String> {
    if frames.is_empty() {
        return Err("no frames".into());
    }
    let caption_h = match &params.caption {
        Some(c) if !c.white.is_empty() || !c.black.is_empty() => params.size / 6,
        _ => 0,
    };
    let settings = gifski::Settings {
        width: Some(params.size),
        height: Some(params.size + caption_h),
        fast: true,
        ..Default::default()
    };
    let (collector, writer) = gifski::new(settings).map_err(|e| e.to_string())?;
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let wb = SharedBuf(buf.clone());
    let writer_thread = std::thread::spawn(move || {
        let _ = writer.write(wb, &mut gifski::progress::NoProgress {});
    });

    let mut ts = 0.0f64;
    for (i, f) in frames.iter().enumerate() {
        let pos = fen::parse(&f.fen, &params.board).map_err(|e| e.to_string())?;
        let p = Params {
            pos,
            board: params.board,
            size: params.size,
            theme: params.theme,
            flat: params.flat,
            flipped: params.flipped,
            last_move: f.last_move,
            squares: vec![],
            arrows: vec![],
            notation: crate::render::Notation::None,
            border: false,
            margin: 0,
            transparent: false,
            caption: params.caption.clone(),
        };
        let pm = raster(&p);
        let img = imgref::ImgVec::new(
            pm.data()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|px| rgb::RGBA::new(px[0], px[1], px[2], px[3]))
                .collect(),
            pm.width() as usize,
            pm.height() as usize,
        );
        // presentation_timestamp = when this frame is shown (cumulative, seconds)
        collector
            .add_frame_rgba(i, img, ts)
            .map_err(|e| e.to_string())?;
        ts += f64::from(params.delay_ms.clamp(100, 3000)) / 1000.0;
    }
    drop(collector); // signals the writer to finish
    writer_thread.join().map_err(|_| "writer thread panicked")?;
    let data = buf.lock().unwrap().clone();
    if data.is_empty() {
        return Err("gif writer produced no data".into());
    }
    // GIF Comment Extension (SPEC §4.5: the comment travels inside the
    // file metadata). Per GIF89a the Logical Screen Descriptor (and the
    // global color table, when present) MUST immediately follow the
    // header, so the comment is inserted after LSD+GCT, never between
    // header and LSD (that made strict viewers reject the file).
    let data = if params.comment.is_empty() {
        data
    } else {
        let lsd_end = {
            let packed = data[10]; // LSD packed field
            let gct = if packed & 0x80 != 0 {
                3 * (1usize << ((packed & 0x07) + 1))
            } else {
                0
            };
            6 + 7 + gct
        };
        let mut out = data[..lsd_end].to_vec();
        out.extend_from_slice(&[0x21, 0xFE]); // extension + comment label
        for chunk in params.comment.as_bytes().chunks(255) {
            out.push(chunk.len() as u8);
            out.extend_from_slice(chunk);
        }
        out.push(0); // block terminator
        out.extend_from_slice(&data[lsd_end..]);
        out
    };
    Ok(data)
}
