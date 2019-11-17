use crate::decoder::{decode_cptv3_header, decode_frame};
use cptv_common::{Cptv3Header, CptvFrame};
use log::Level;
#[allow(unused)]
use log::{info, trace, warn};
use std::cell::RefCell;
use std::ops::Range;
use wasm_bindgen::__rt::std::io::Cursor;
use wasm_bindgen::prelude::*;
use zstd_rs::frame_decoder;

mod decoder;

// The global allocator used by wasm code
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

struct PlaybackInfo {
    offset_in_block: usize,
    prev_block: usize,
    prev_frame: usize,
}

impl PlaybackInfo {
    pub fn new() -> PlaybackInfo {
        PlaybackInfo {
            offset_in_block: 0,
            prev_block: 0,
            prev_frame: 0,
        }
    }
}

struct DownloadedData {
    bytes: Vec<u8>,
    ranges: Vec<Range<usize>>,
}
impl DownloadedData {
    pub fn new() -> DownloadedData {
        DownloadedData {
            bytes: Vec::new(),
            ranges: Vec::new(),
        }
    }
}

thread_local! {
    static RAW_FILE_DATA: RefCell<DownloadedData> = RefCell::new(DownloadedData::new());
}

thread_local! {
    static CLIP_INFO: RefCell<Cptv3Header> = RefCell::new(Cptv3Header::new());
}
thread_local! {
    static IFRAME_BLOCKS: RefCell<Vec<Vec<u8>>> = RefCell::new(Vec::new());
}
thread_local! {
    static FRAME_BUFFER: RefCell<CptvFrame> = RefCell::new(CptvFrame::new());
}

thread_local! {
    static PLAYBACK_INFO: RefCell<PlaybackInfo> = RefCell::new(PlaybackInfo::new());
}

fn decode_zstd_blocks(meta: &Cptv3Header, remaining: &[u8]) -> Vec<Vec<u8>> {
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
    let mut decoded_zstd_blocks = Vec::new();
    for iframe_block in iframe_blocks {
        let mut frame_dec = frame_decoder::FrameDecoder::new();
        let mut f = Cursor::new(iframe_block);
        frame_dec.init(&mut f).unwrap();
        frame_dec
            .decode_blocks(&mut f, frame_decoder::BlockDecodingStrategy::All)
            .unwrap();
        if let Some(result) = frame_dec.collect() {
            decoded_zstd_blocks.push(result);
        }
    }
    decoded_zstd_blocks
}

#[wasm_bindgen]
pub fn init_buffer_with_size(size: usize) -> Result<(), JsValue> {
    // Init the console logging stuff on startup, so that wasm can print things
    // into the browser console.
    console_error_panic_hook::set_once();
    console_log::init_with_level(Level::Debug).unwrap();

    RAW_FILE_DATA.with(|x| {
        x.borrow_mut().bytes = vec![0u8; size];
    });
    Ok(())
}

#[wasm_bindgen]
pub fn insert_chunk_at_offset(chunk: &[u8], offset: usize) -> Result<(), JsValue> {
    RAW_FILE_DATA.with(|x| {
        let range = offset..offset + chunk.len();
        let download_data = &mut x.borrow_mut();
        let target_slice = &mut download_data.bytes[range.clone()];
        target_slice.copy_from_slice(chunk);
        download_data.ranges.push(range);
    });
    Ok(())
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = loadedHeaderInfo)]
    fn loaded_header_info();

    #[wasm_bindgen(js_name = cancelLoading)]
    fn cancel_loading();
}

#[wasm_bindgen]
pub fn init_with_cptv_data(input: &[u8]) -> Result<(), JsValue> {
    // TODO(jon): Calculate how much we need to buffer in order to stream, and keep adjusting that estimate.
    if let Ok((remaining, meta)) = decode_cptv3_header(&input) {
        let range_degrees_c = 150.0;
        let max_val = 16384;
        let min = meta.min_value as f64;
        let max = meta.max_value as f64;
        let f = range_degrees_c / max_val as f64;
        let min_c = -10.0 + (f * min);
        let max_c = -10.0 + (f * max);
        //info!("temp {}C - {}c", min_c, max_c);

        let zstd_blocks = decode_zstd_blocks(&meta, remaining);
        IFRAME_BLOCKS.with(|x| *x.borrow_mut() = zstd_blocks);
        CLIP_INFO.with(|x| *x.borrow_mut() = meta);
    }
    Ok(())
}

#[wasm_bindgen]
pub fn get_num_frames() -> u32 {
    CLIP_INFO.with(|x| x.borrow().num_frames)
}

#[wasm_bindgen]
pub fn get_width() -> u32 {
    CLIP_INFO.with(|x| x.borrow().v2.width)
}

#[wasm_bindgen]
pub fn get_height() -> u32 {
    CLIP_INFO.with(|x| x.borrow().v2.height)
}

#[wasm_bindgen]
pub fn get_frame_rate() -> u8 {
    CLIP_INFO.with(|x| x.borrow().frame_rate)
}

#[wasm_bindgen]
pub fn get_frames_per_iframe() -> u8 {
    CLIP_INFO.with(|x| x.borrow().frames_per_iframe)
}

#[wasm_bindgen]
pub fn get_min_value() -> u16 {
    CLIP_INFO.with(|x| x.borrow().min_value)
}

#[wasm_bindgen]
pub fn get_max_value() -> u16 {
    CLIP_INFO.with(|x| x.borrow().max_value)
}

#[wasm_bindgen]
pub fn queue_frame(number: u32, callback: JsValue) -> bool {
    // Scrub to frame `number`.
    // If we have loaded everything, just return true.
    RAW_FILE_DATA.with(|x| {
        let downloaded_data = x.borrow();

        // Work out where frame `number` would be in terms of bytes, then search ranges for that byte offset.
        // Cancel any current download if it's not immediately getting us the bytes we need.

        // If there are any ranges after `number` that haven't downloaded, queue them for download from the
        // earliest un-downloaded offset.

        // Stick the frame we want in a pending playback frame variable.
        // Later, when the byte range we want comes in, and we have enough buffered, signal the front-end
        // to get the frame, and continue playing.

        // We also need to book-keep what the start offset is for the range we're currently downloading, if any.
    });
    // cancel_loading();
    true
}

#[wasm_bindgen]
pub fn get_frame(number: u32, image_data: &mut [u8]) {
    // Find the block closest, decode from the start to frame x:
    let (mut offset, prev_block, prev_frame_num) = PLAYBACK_INFO.with(|info| {
        let info = info.borrow();
        (info.offset_in_block, info.prev_block, info.prev_frame)
    });
    let prev_frame_num = if number as usize != prev_frame_num {
        // We got a seek.
        number as usize
    } else {
        prev_frame_num
    };
    let (max, min, frames_per_iframe, num_frames, num_blocks) = CLIP_INFO.with(|meta| {
        let meta = meta.borrow();
        (
            meta.max_value,
            meta.min_value,
            meta.frames_per_iframe,
            meta.num_frames,
            meta.toc.len(),
        )
    });
    let block_num = (prev_frame_num as u32 / frames_per_iframe as u32) as usize;
    if block_num != prev_block {
        offset = 0;
    }
    let inv_dynamic_range = 1.0 / (max - min) as f32;
    IFRAME_BLOCKS.with(|data| {
        let block = &data.borrow()[block_num];
        // Read the frame out of the data:
        FRAME_BUFFER.with(|prev_frame| {
            let frame = {
                if let Ok((remaining, frame)) =
                    decode_frame(&prev_frame.borrow(), &block[offset..], offset == 0)
                {
                    let image = &frame.image_data;
                    // Copy frame out to output:
                    let mut i = 0;
                    for y in 0..image.height() {
                        for x in 0..image.width() {
                            let val = ((image[y][x] as u16 - min) as f32
                                * inv_dynamic_range
                                * 255.0) as u8;
                            image_data[i + 0] = val;
                            image_data[i + 1] = val;
                            image_data[i + 2] = val;
                            image_data[i + 3] = 255;
                            i += 4;
                        }
                    }
                    Some((remaining, frame))
                } else {
                    None
                }
            };
            if let Some((remaining, frame)) = frame {
                PLAYBACK_INFO.with(|info| {
                    let mut info = info.borrow_mut();
                    info.offset_in_block = block.len() - remaining.len();
                    info.prev_block = block_num;
                    let next_frame = usize::min(num_frames as usize, prev_frame_num + 1);
                    info.prev_frame = next_frame;
                });
                *prev_frame.borrow_mut() = frame;
            }
        });
    });
}
