use crate::decoder::{
    decode_cptv_header, decode_frame, decode_frame_v2, get_dynamic_range, CptvHeader,
};
use cptv_common::{Cptv2Header, Cptv3Header, CptvFrame};
use js_sys::Uint8Array;
use log::Level;
#[allow(unused)]
use log::{info, trace, warn};
use std::alloc::System;
use std::cell::RefCell;
use std::io::Read;
use std::ops::Range;
use wasm_bindgen::__rt::std::io::Cursor;
use wasm_bindgen::prelude::*;
use libflate::deflate::Decoder;
use crate::v2::FrameHeaderV2;
//use wasm_tracing_allocator::WasmTracingAllocator;

mod decoder;

#[cfg(feature = "cptv2-support")]
mod v2;
#[cfg(feature = "cptv3-support")]
mod v3;

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

pub struct CptvPlayerContext {
    // Holds information about the current play-head
    playback_info: PlaybackInfo,

    // Holds information about current downloaded file data (including a partial map if we're streaming and seeking)
    downloaded_data: DownloadedData,

    // Current clip metadata
    clip_info: CptvHeader,

    // Raw frame data blocks, in compressed units.  CPTV2 has only a single compressed unit
    // - it must be played back from the beginning of the file and each frame is dependant on the previous.
    iframe_blocks: Vec<Vec<u8>>,

    // Current decoded frame data.
    frame_buffer: CptvFrame,

    // Decoder - once we know what kind of file we have, we store a trait object for each decoder type?
}

#[wasm_bindgen]
impl CptvPlayerContext {
    pub fn new() -> CptvPlayerContext {
        // Init the console logging stuff on startup, so that wasm can print things
        // into the browser console.
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(Level::Debug).unwrap();
        CptvPlayerContext {
            playback_info: PlaybackInfo::new(),
            downloaded_data: DownloadedData::new(),
            clip_info: CptvHeader::UNINITIALISED,
            iframe_blocks: Vec::new(),
            frame_buffer: CptvFrame::new()
        }
    }

    #[wasm_bindgen(js_name = initWithCptvData)]
    pub fn init_with_cptv_data(&mut self, input: &[u8]) -> Result<JsValue, JsValue> {
        if input.len() > 0 {
            // See if the input is gzip encoded:  If it is, we probably have a cptv2 file.

            // TODO(jon): This also needs to work with streaming input though...

            #[cfg(feature = "cptv2-support")]
                let input = if input[0] == 0x1f && input[1] == 0x8b {
                // It's a gzipped stream.
                let mut decoded = Vec::new();
                let mut gz_decoder = Decoder::new(&input[..]).unwrap();
                gz_decoder.read_to_end(&mut decoded).unwrap();
                decoded
            } else {
                input
            };

            #[cfg(not(feature = "cptv2-support"))]
                let input = if input[0] == 0x1f && input[1] == 0x8b {
                // It's a gzipped stream, which we don't support if only cptv3-support is enabled.
                panic!("Unsupported file is probably cptv2 format");
            } else {
                input
            };

            self.frame_buffer = CptvFrame::new();
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
                        self.iframe_blocks = decode_zstd_blocks(&meta, remaining);
                    }
                    CptvHeader::V2(_) => {
                        self.iframe_blocks = vec![remaining.to_vec()];
                    }
                    _ => panic!("uninitialised"),
                }
                self.clip_info = meta;
                self.playback_info = PlaybackInfo::new();
            }
            Ok(JsValue::from_bool(true))
        } else {
            Err(JsValue::from_bool(false))
        }
    }


    // TODO(jon): Maybe structure this as a CPTVDecode struct with a playback trait, and do dynamic dispatch depending on CPTV versions?

    #[wasm_bindgen(js_name = getFrame)]
    pub fn get_frame(&self, number: u32, image_data: &mut [u8]) -> bool {
        match self.clip_info {
            CptvHeader::V2(_) => get_frame_v2(number, image_data),
            CptvHeader::V3(_) => get_frame_v3(number, image_data),
            CptvHeader::UNINITIALISED => false,
        }
    }


    // TODO(jon): Not sure that this does the right thing?
    #[wasm_bindgen(js_name = getRawFrame)]
    pub fn get_raw_frame(&self, image_data: &mut [u8]) -> FrameHeaderV2 {
        match self.clip_info {
            CptvHeader::V2(_) => get_raw_frame_v2(image_data),
            _ => unreachable!(),
        }
    }

    #[wasm_bindgen(js_name = getNumFrames)]
    pub fn get_num_frames(&self) -> u32 {
        match &self.clip_info {
            CptvHeader::V2(h) => 0,
            CptvHeader::V3(h) => h.num_frames,
            _ => panic!("uninitialised"),
        }
    }

    #[wasm_bindgen(js_name = getWidth)]
    pub fn get_width(&self) -> u32 {
        match &self.clip_info {
            CptvHeader::V2(h) => h.width,
            CptvHeader::V3(h) => h.v2.width,
            _ => panic!("uninitialised"),
        }
    }

    #[wasm_bindgen(js_name = getHeight)]
    pub fn get_height(&self) -> u32 {
        match &self.clip_info {
            CptvHeader::V2(h) => h.height,
            CptvHeader::V3(h) => h.v2.height,
            _ => panic!("uninitialised"),
        }
    }

    #[wasm_bindgen(js_name = getFrameRate)]
    pub fn get_frame_rate(&self) -> u8 {
        match &self.clip_info {
            CptvHeader::V2(h) => 9,
            CptvHeader::V3(h) => h.frame_rate,
            _ => panic!("uninitialised"),
        }
    }

    #[wasm_bindgen(js_name = getFramesPerIframe)]
    pub fn get_frames_per_iframe(&self) -> u8 {
        match &self.clip_info {
            CptvHeader::V2(h) => 1,
            CptvHeader::V3(h) => h.frames_per_iframe,
            _ => panic!("uninitialised"),
        }
    }

    #[wasm_bindgen(js_name = getMinValue)]
    pub fn get_min_value(&self) -> u16 {
        match &self.clip_info {
            CptvHeader::V2(h) => 0,
            CptvHeader::V3(h) => h.min_value,
            _ => panic!("uninitialised"),
        }
    }

    #[wasm_bindgen(js_name = getMaxValue)]
    pub fn get_max_value(&self) -> u16 {
        match &self.clip_info {
            CptvHeader::V2(h) => u16::MAX,
            CptvHeader::V3(h) => h.max_value,
            _ => panic!("uninitialised"),
        }
    }

    #[wasm_bindgen(js_name = getHeader)]
    pub fn get_header(&self) -> JsValue {
        match &self.clip_info {
            CptvHeader::V2(h) => JsValue::from_str(&format!("{:#?}", h)),
            CptvHeader::V3(h) => JsValue::from_str(&format!("{:#?}", h)),
            _ => JsValue::from_str("Unable to parse header"),
        }
    }

    #[wasm_bindgen(js_name = initBufferWithSize)]
    pub fn init_buffer_with_size(&mut self, size: usize) -> Result<(), JsValue> {
        self.downloaded_data.bytes = vec![u8; size];
        Ok(())
    }

    #[wasm_bindgen(js_name = insertChunkAtOffset)]
    pub fn insert_chunk_at_offset(&mut self, chunk: &[u8], offset: usize) -> Result<(), JsValue> {
        let range = offset..offset + chunk.len();
        let target_slice = &mut self.download_data.bytes[range.clone()];
        target_slice.copy_from_slice(chunk);
        self.download_data.ranges.push(range);
        Ok(())
    }


    #[wasm_bindgen(js_name = queueFrame)]
    pub fn queue_frame(&self, number: u32, callback: JsValue) -> bool {
        // Scrub to frame `number`.
        // If we have loaded everything, just return true.
        //RAW_FILE_DATA.with(|x| {
        //    let downloaded_data = x.borrow();

            // Work out where frame `number` would be in terms of bytes, then search ranges for that byte offset.
            // Cancel any current download if it's not immediately getting us the bytes we need.

            // If there are any ranges after `number` that haven't downloaded, queue them for download from the
            // earliest un-downloaded offset.

            // Stick the frame we want in a pending playback frame variable.
            // Later, when the byte range we want comes in, and we have enough buffered, signal the front-end
            // to get the frame, and continue playing.

            // We also need to book-keep what the start offset is for the range we're currently downloading, if any.
        //});
        // cancel_loading();
        true
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = loadedHeaderInfo)]
    fn loaded_header_info();

    #[wasm_bindgen(js_name = cancelLoading)]
    fn cancel_loading();
}
