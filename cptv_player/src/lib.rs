use js_sys::Uint8Array;
use log::Level;
use wasm_bindgen::prelude::*;

use crate::decoder::decode_cptv3;
#[allow(unused)]
use log::{info, trace, warn};

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

    let frames = decode_cptv3(&input);
    // Now decode and play frames!

    /*
        let mut f = File::open(path).unwrap();
    let mut frame_dec = frame_decoder::FrameDecoder::new();
    frame_dec.init(&mut f).unwrap();
    frame_dec.decode_blocks(&mut f, frame_decoder::BlockDecodingStrategy::All).unwrap();

    // result contains the whole decoded file
    let result = frame_dec.collect();
        */

    Ok(())
}
