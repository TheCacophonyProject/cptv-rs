use crate::decoder::{
    decode_cptv_header, decode_frame, decode_frame_v2, get_dynamic_range, CptvHeader,
};
use cptv_common::{Cptv2Header, Cptv3Header, CptvFrame};
use js_sys::Uint8Array;
use libflate::gzip::Decoder;
use log::Level;
#[allow(unused)]
use log::{info, trace, warn};
use ruzstd::frame_decoder;
use std::alloc::System;
use std::cell::RefCell;
use std::io::Read;
use std::ops::Range;
use wasm_bindgen::__rt::std::io::Cursor;
use wasm_bindgen::prelude::*;
//use wasm_tracing_allocator::WasmTracingAllocator;

mod decoder;

// The global allocator used by wasm code
// #[global_allocator]
// static ALLOC: wasm_tracing_allocator::WasmTracingAllocator<System> = WasmTracingAllocator(System);

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

// TODO(jon): Make this multi-threaded in wasm, so you can run multiple versions concurrently?

thread_local! {
    static RAW_FILE_DATA: RefCell<DownloadedData> = RefCell::new(DownloadedData::new());
}

thread_local! {
    static CLIP_INFO: RefCell<CptvHeader> = RefCell::new(CptvHeader::UNINITIALISED);
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

#[wasm_bindgen(js_name = initBufferWithSize)]
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

#[wasm_bindgen(js_name = insertChunkAtOffset)]
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

#[wasm_bindgen(js_name = initWithCptvData)]
pub fn init_with_cptv_data(input: &[u8]) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(Level::Debug);
    if input.len() > 0 {
        // See if the input is gzip encoded:
        let mut decoded = Vec::new();
        let input = if input[0] == 0x1f && input[1] == 0x8b {
            // It's a gzipped stream.
            let mut gz_decoder = Decoder::new(&input[..]).unwrap();
            gz_decoder.read_to_end(&mut decoded).unwrap();
            &decoded
        } else {
            input
        };

        FRAME_BUFFER.with(|prev_frame| {
            *prev_frame.borrow_mut() = CptvFrame::new();
        });
        // TODO(jon): Calculate how much we need to buffer in order to stream, and keep adjusting that estimate.
        if let Ok((remaining, meta)) = decode_cptv_header(&input) {
            match &meta {
                CptvHeader::V3(meta) => {
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
                }
                CptvHeader::V2(_) => {
                    IFRAME_BLOCKS.with(|x| *x.borrow_mut() = vec![remaining.to_vec()]);
                }
                _ => panic!("uninitialised"),
            }
            CLIP_INFO.with(|x| *x.borrow_mut() = meta);
            PLAYBACK_INFO.with(|x| *x.borrow_mut() = PlaybackInfo::new());
        }
        Ok(JsValue::from_bool(true))
    } else {
        Err(JsValue::from_bool(false))
    }
}

#[wasm_bindgen(js_name = getNumFrames)]
pub fn get_num_frames() -> u32 {
    CLIP_INFO.with(|x| {
        let header = &*x.borrow();
        match header {
            CptvHeader::V2(h) => 0,
            CptvHeader::V3(h) => h.num_frames,
            _ => panic!("uninitialised"),
        }
    })
}

#[wasm_bindgen(js_name = getWidth)]
pub fn get_width() -> u32 {
    CLIP_INFO.with(|x| {
        let header = &*x.borrow();
        match header {
            CptvHeader::V2(h) => h.width,
            CptvHeader::V3(h) => h.v2.width,
            _ => panic!("uninitialised"),
        }
    })
}

#[wasm_bindgen(js_name = getHeight)]
pub fn get_height() -> u32 {
    CLIP_INFO.with(|x| {
        let header = &*x.borrow();
        match header {
            CptvHeader::V2(h) => h.height,
            CptvHeader::V3(h) => h.v2.height,
            _ => panic!("uninitialised"),
        }
    })
}

#[wasm_bindgen(js_name = getFrameRate)]
pub fn get_frame_rate() -> u8 {
    CLIP_INFO.with(|x| {
        let header = &*x.borrow();
        match header {
            CptvHeader::V2(h) => 9,
            CptvHeader::V3(h) => h.frame_rate,
            _ => panic!("uninitialised"),
        }
    })
}

#[wasm_bindgen(js_name = getFramesPerIframe)]
pub fn get_frames_per_iframe() -> u8 {
    CLIP_INFO.with(|x| {
        let header = &*x.borrow();
        match header {
            CptvHeader::V2(h) => 1,
            CptvHeader::V3(h) => h.frames_per_iframe,
            _ => panic!("uninitialised"),
        }
    })
}

#[wasm_bindgen(js_name = getMinValue)]
pub fn get_min_value() -> u16 {
    CLIP_INFO.with(|x| {
        let header = &*x.borrow();
        match header {
            CptvHeader::V2(h) => 0,
            CptvHeader::V3(h) => h.min_value,
            _ => panic!("uninitialised"),
        }
    })
}

#[wasm_bindgen(js_name = getMaxValue)]
pub fn get_max_value() -> u16 {
    CLIP_INFO.with(|x| {
        let header = &*x.borrow();
        match header {
            CptvHeader::V2(h) => u16::MAX,
            CptvHeader::V3(h) => h.max_value,
            _ => panic!("uninitialised"),
        }
    })
}

#[wasm_bindgen(js_name = getHeader)]
pub fn get_header() -> JsValue {
    CLIP_INFO.with(|x| {
        let header = &*x.borrow();
        match header {
            CptvHeader::V2(h) => JsValue::from_str(&format!("{:#?}", h)),
            CptvHeader::V3(h) => JsValue::from_str(&format!("{:#?}", h)),
            _ => JsValue::from_str("Unable to parse header"),
        }
    })
}

#[wasm_bindgen(js_name = queueFrame)]
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
pub struct FrameHeaderV2 {
    pub time_on: u32,
    pub last_ffc_time: u32,
    pub frame_number: u32,
    pub has_next_frame: bool,
}

fn get_raw_frame_v2(image_data: &mut [u8]) -> FrameHeaderV2 {
    let mut frame_header = FrameHeaderV2 {
        time_on: 0,
        last_ffc_time: 0,
        frame_number: 0,
        has_next_frame: false,
    };

    IFRAME_BLOCKS.with(|data| {
        // We only use the first block for V2
        let offset = PLAYBACK_INFO.with(|info| {
            let info = &*info.borrow();
            info.offset_in_block
        });
        let block = &data.borrow()[0];
        //info!("Get frame {}", number);
        // Read the frame out of the data:

        FRAME_BUFFER.with(|prev_frame| {
            let frame = {
                assert!(
                    offset <= block.len(),
                    "Offset is wrong {} vs {}",
                    offset,
                    block.len()
                );
                if let Ok((remaining, frame)) =
                    decode_frame_v2(&prev_frame.borrow(), &block[offset..])
                {
                    image_data.copy_from_slice(frame.image_data.as_slice());
                    Some((remaining, frame))
                } else {
                    None
                }
            };
            if let Some((remaining, frame)) = frame {
                PLAYBACK_INFO.with(|info| {
                    let mut info = info.borrow_mut();
                    assert!(block.len() > remaining.len());
                    let offset = block.len() - remaining.len();
                    info.offset_in_block = usize::min(block.len(), offset);
                    info.prev_block = 0;
                    frame_header.last_ffc_time = frame.last_ffc_time;
                    frame_header.time_on = frame.time_on;
                    frame_header.frame_number = info.prev_frame as u32;
                    info.prev_frame += 1;
                });
                *prev_frame.borrow_mut() = frame;
            } else {
                *prev_frame.borrow_mut() = CptvFrame::new();
                PLAYBACK_INFO.with(|info| {
                    let mut info = info.borrow_mut();
                    // Restart playback at the beginning.
                    info.offset_in_block = 0;
                    info.prev_frame = 0;
                });
            }
        });
    });
    frame_header
}

fn get_frame_v2(number: u32, image_data: &mut [u8]) -> bool {
    let mut has_next_frame = false;
    IFRAME_BLOCKS.with(|data| {
        // We only use the first block for V2
        let offset = PLAYBACK_INFO.with(|info| {
            let info = &*info.borrow();
            info.offset_in_block
        });
        let block = &data.borrow()[0];
        //info!("Get frame {}", number);
        // Read the frame out of the data:

        FRAME_BUFFER.with(|prev_frame| {
            let frame = {
                assert!(
                    offset <= block.len(),
                    "Offset is wrong {} vs {}",
                    offset,
                    block.len()
                );
                if let Ok((remaining, frame)) =
                    decode_frame_v2(&prev_frame.borrow(), &block[offset..])
                {
                    let image = &frame.image_data;
                    // Copy frame out to output:
                    let range = get_dynamic_range(image);
                    let min = *range.start() as u16;
                    let max = *range.end() as u16;
                    let inv_dynamic_range = 1.0 / (max - min) as f32;
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
                    assert!(block.len() > remaining.len());
                    let offset = block.len() - remaining.len();
                    info.offset_in_block = usize::min(block.len(), offset);
                    info.prev_block = 0;
                    let next_frame = number + 1;
                    info.prev_frame = next_frame as usize;
                });
                has_next_frame = true;
                *prev_frame.borrow_mut() = frame;
            } else {
                *prev_frame.borrow_mut() = CptvFrame::new();
                PLAYBACK_INFO.with(|info| {
                    let mut info = info.borrow_mut();
                    info.offset_in_block = 0;
                });
            }
        });
    });
    has_next_frame
}

fn get_frame_v3(number: u32, image_data: &mut [u8]) -> bool {
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
    // let (max, min, frames_per_iframe, num_frames, num_blocks) = CLIP_INFO.with(|meta| {
    //     let meta = meta.borrow();
    //     (
    //         meta.max_value,
    //         meta.min_value,
    //         meta.frames_per_iframe,
    //         meta.num_frames,
    //         meta.toc.len(),
    //     )
    // });
    let (max, min, frames_per_iframe, num_frames) = (
        get_max_value(),
        get_min_value(),
        get_frames_per_iframe(),
        get_num_frames(),
    );
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
    true
}

#[wasm_bindgen(js_name = getFrame)]
pub fn get_frame(number: u32, image_data: &mut [u8]) -> bool {
    CLIP_INFO.with(|meta| {
        let meta = &*meta.borrow();
        match meta {
            CptvHeader::V2(_) => get_frame_v2(number, image_data),
            CptvHeader::V3(_) => get_frame_v3(number, image_data),
            CptvHeader::UNINITIALISED => false,
        }
    })
}

#[wasm_bindgen(js_name = getRawFrame)]
pub fn get_raw_frame(image_data: &mut [u8]) -> FrameHeaderV2 {
    CLIP_INFO.with(|meta| {
        let meta = &*meta.borrow();
        match meta {
            CptvHeader::V2(_) => get_raw_frame_v2(image_data),
            _ => unreachable!(),
        }
    })
}
