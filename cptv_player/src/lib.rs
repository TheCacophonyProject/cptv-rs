use js_sys::Uint8Array;
use log::Level;
use wasm_bindgen::prelude::*;

use crate::decoder::decode_cptv3_header;
#[allow(unused)]
use log::{info, trace, warn};
use wasm_bindgen::__rt::std::io::Cursor;
use zstd_rs::frame_decoder;

mod decoder;

// The global allocator used by wasm code
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn init_with_cptv_data(buffer: Uint8Array) -> Result<(), JsValue> {
    // Init the console logging stuff on startup, so that wasm can print things
    // into the browser console.
    console_error_panic_hook::set_once();
    console_log::init_with_level(Level::Debug).unwrap();
    info!("Hello from wasm");
    info!("Got buffer {}", buffer.length());

    let mut input: Vec<u8> = vec![0u8; buffer.length() as usize];
    buffer.copy_to(&mut input);
    info!("Input len {}", input.len());

    if let Some((remaining, meta)) = decode_cptv3_header(&input).ok() {
        let mut iframe_blocks = Vec::new();
        let mut prev_offset = 0;

        for pos in &meta.toc[1..] {
            let pos = *pos as usize;
            iframe_blocks.push(&remaining[prev_offset..pos]);
            prev_offset = pos;
        }
        iframe_blocks.push(&remaining[prev_offset..]);

        // Should we just decode frame blocks on demand, or up front?
        // Now decode and play frames!

        // Event loop here to request decoding and drawing of frames.
        // What is the best way to listen for requests from the UI?
        // Probably a request animation frame loop, right?

        // Maybe all of this just goes into a const Mutex<Vec<u8>> etc?

        for iframe_block in iframe_blocks {
            info!("Block {}", iframe_block.len());
            let mut frame_dec = frame_decoder::FrameDecoder::new();
            let mut f = Cursor::new(iframe_block);
            frame_dec.init(&mut f);
            frame_dec
                .decode_blocks(&mut f, frame_decoder::BlockDecodingStrategy::All)
                .unwrap();

            // result contains the whole decoded file
            if let Some(result) = frame_dec.collect() {
                info!("Decoded into {}", result.len());
                // Now decode the frames, and write to canvas!
            }
        }
    }

    Ok(())
}
