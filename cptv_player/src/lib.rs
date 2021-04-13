use crate::decoder::{decode_cptv_header, decode_frame, decode_frame_v2, get_dynamic_range, CptvHeader, decode_frame_header_v2, unpack_frame_v2};
use cptv_common::{Cptv2Header, Cptv3Header, CptvFrame, FrameData};
use js_sys::{Uint8Array, Uint16Array, Reflect};
use log::Level;
#[allow(unused)]
use log::{info, trace, warn};
use std::alloc::System;
use std::cell::RefCell;
use std::io::{Read, ErrorKind};
use std::ops::Range;
use wasm_bindgen::__rt::std::io::Cursor;
use wasm_bindgen::prelude::*;

#[cfg(feature = "cptv2-support")]
use libflate::non_blocking::gzip::Decoder;
use crate::v2::{FrameHeaderV2, get_raw_frame_v2, get_frame_v2, decode_cptv2_header};

#[cfg(feature = "cptv3-support")]
use crate::v3::decode_zstd_blocks;
#[cfg(feature = "cptv3-support")]
use crate::v3::get_frame_v3;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use js_sys::Reflect::construct_with_new_target;
use std::io;
use wasm_bindgen_futures::future_to_promise;
//use wasm_tracing_allocator::WasmTracingAllocator;

mod decoder;

#[cfg(feature = "cptv2-support")]
mod v2;
#[cfg(feature = "cptv3-support")]
mod v3;

struct PlaybackInfo {
    offset_in_block: usize,
    total_frames: Option<usize>,
    prev_block: usize,
    prev_frame: usize,
    min_value: u16,
    max_value: u16,
}

impl PlaybackInfo {
    pub fn new() -> PlaybackInfo {
        PlaybackInfo {
            offset_in_block: 0,
            total_frames: None,
            prev_block: 0,
            prev_frame: 0,
            min_value: u16::MAX,
            max_value: u16::MIN,
        }
    }
}

struct DownloadedData {
    bytes: Option<ResumableReader>,
    decoded: Vec<u8>,
    first_frame_offset: Option<usize>,
    stream_ended: bool,
    gzip_ended: bool,
    num_decompressed_bytes: usize,
    latest_frame_offset: Option<usize>,

    // We probably want to make CPTV2 files work with our ranges queries, since we'll want that
    // for V3 files where we can have multiple partial bits of the file downloaded?
    ranges: Vec<Range<usize>>,
}

impl DownloadedData {
    pub fn new() -> DownloadedData {
        DownloadedData {
            bytes: None,
            decoded: vec![0;100],
            first_frame_offset: None,
            stream_ended: false,
            gzip_ended: false,
            latest_frame_offset: None,
            num_decompressed_bytes: 0,
            ranges: Vec::new(),
        }
    }

    pub fn frame_data_mut(&mut self) -> Option<&mut [u8]> {
        match self.first_frame_offset {
            Some(offset) => Some(&mut self.decoded[offset..self.num_decompressed_bytes]),
            None => None
        }
    }

    pub fn frame_data(&self) -> Option<&[u8]> {
        match self.first_frame_offset {
            Some(offset) => Some(&self.decoded[offset..self.num_decompressed_bytes]),
            None => None
        }
    }
}

struct ResumableReader {
    inner: Cursor<Vec<u8>>, // Initialise to the total number of bytes, which we know from the request header.
    available: usize, // Every time we add a chunk, advance this to the end
    used: usize, // Every time we read bytes, advance this to the amount of read bytes.
    stream_ended: bool,
}

impl ResumableReader {
    pub fn new_with_capacity(size: usize) -> ResumableReader {
        ResumableReader {
            inner: Cursor::new(vec![0; size]),
            available: 0,
            used: 0,
            stream_ended: false
        }
    }

    pub fn append_bytes(&mut self, bytes: &Uint8Array) {
        assert!(bytes.byte_length() == bytes.length());
        assert!(self.available + bytes.length() as usize <= self.inner.get_ref().len());
        bytes.copy_to(&mut self.inner.get_mut()[self.available..self.available + bytes.byte_length() as usize]);
        self.available += bytes.length() as usize;
        //info!("Bytes available to nom/gzip decode {}", self.available);
        // if self.available == self.inner.get_ref().capacity() {
        //     info!("!! Filled download buffer to expected size");
        // }
    }
}

impl Read for ResumableReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.used == self.available && self.available < self.inner.get_ref().len() {
            info!("called read with available {}, used: {}", self.available, self.used);
            Err(io::Error::new(io::ErrorKind::WouldBlock, "Waiting for more bytes"))
        } else if buf.is_empty() {
            info!("Got zero bytes, need to allocate into read buffer");
            Ok(0)
        } else {
            let would_be_used = self.used + buf.len();
            if would_be_used >= self.available {
                //info!("Trying to read over");
                if self.stream_ended {
                     return Ok(0);
                }
                return Err(io::Error::new(io::ErrorKind::WouldBlock, "Waiting for more bytes"))
            }
            let read_bytes = match self.inner.read(&mut buf[..]) {
                Ok(r) => {
                    if self.used + r >= self.available {
                        info!("Reached end of available bytes");
                    }
                    Ok(r)
                },
                Err(r) => {
                    info!("Got error {:?}", r);
                    Err(r)
                }
            }?;
            self.used += read_bytes;
            if read_bytes == 0 {
                info!("Got zero bytes");
            }
            Ok(read_bytes)
        }
    }
}

trait Playback {
    fn init_with_cptv_data(&mut self, input: &[u8]) -> Result<JsValue, JsValue>;

    // TODO(jon): All of these functions should be on a context.
    // The context is either uninitialised, V2, V3
    fn get_frame();
    fn get_raw_frame();
    fn get_frame_rate();
    fn get_min_value();
    fn get_max_value();
    fn get_header();
    fn get_num_frames();
}

#[wasm_bindgen]
extern "C" {
    pub type ReadableStreamDefaultReader;

    # [wasm_bindgen (catch , method , structural , js_class = "ReadableStreamDefaultReader" , js_name = cancel)]
    pub fn cancel(this: &ReadableStreamDefaultReader) -> Result<js_sys::Promise, JsValue>;
    # [wasm_bindgen (catch , method , structural , js_class = "ReadableStreamDefaultReader" , js_name = read)]
    pub fn read(this: &ReadableStreamDefaultReader) -> Result<js_sys::Promise, JsValue>;
}

#[wasm_bindgen]
pub struct CptvPlayerContext {
    // Holds information about the current play-head
    playback_info: PlaybackInfo,

    // Holds information about current downloaded file data (including a partial map if we're streaming and seeking)
    downloaded_data: DownloadedData,

    // Current clip metadata
    clip_info: CptvHeader, // TODO(jon): Are we okay with doing our dynamic dispatch off of this enum?

    // Raw frame data blocks, in compressed units.  CPTV2 has only a single compressed unit
    // - it must be played back from the beginning of the file and each frame is dependant on the previous.
    iframe_blocks: Vec<Vec<u8>>,

    // Current decoded frame data - should be the same format for all files
    frame_buffer: CptvFrame,

    // TODO(jon): Keep every 9th frame here in uncompressed form, so that we can seek more efficiently.
    iframes: Vec<CptvFrame>,

    // TODO(jon): Handle skipping background frames

    // Decoder - once we know what kind of file we have, we store a trait object for each decoder type?
    reader: Option<ReadableStreamDefaultReader>,
    gz_decoder: Option<Decoder<ResumableReader>>,
}

#[wasm_bindgen]
impl CptvPlayerContext {

    pub fn init() {
        console_error_panic_hook::set_once();
        let _ = console_log::init_with_level(Level::Debug).unwrap();
    }

    pub fn new() -> CptvPlayerContext {
        // Init the console logging stuff on startup, so that wasm can print things
        // into the browser console.
        CptvPlayerContext {
            playback_info: PlaybackInfo::new(),
            downloaded_data: DownloadedData::new(),
            clip_info: CptvHeader::UNINITIALISED,
            iframe_blocks: Vec::new(),
            iframes: Vec::new(),
            reader: None,
            frame_buffer: CptvFrame::new(),
            gz_decoder: None
        }
    }

    async fn get_bytes(&mut self) -> Result<bool, JsValue> {
        let result = wasm_bindgen_futures::JsFuture::from(self.read_from_stream()).await?;
        let is_last_chunk = Reflect::get(&result, &JsValue::from_str("done")).expect("Should have property 'done'").as_bool().unwrap();
        if !is_last_chunk {
            // TODO(jon): If done is `true` value may not exist - check that all browsers behave this way.
            let value = Reflect::get(&result, &JsValue::from_str("value")).expect("Should have property 'value'");
            let arr = value.dyn_into::<Uint8Array>().unwrap();
            // Get the resumable reader from the gz decoder if it exists
            // TODO(jon): Handle non-gzip streams for cptv3
            if self.gz_decoder.is_some() {
                self.reader_mut().append_bytes(&arr);
            } else {
                let first_pump = self.downloaded_data.bytes.as_ref().unwrap().available == 0;
                if first_pump {
                    self.reader_mut().append_bytes(&arr);
                    let has_gz_stream = self.downloaded_data.bytes.as_ref().unwrap().inner.get_ref()[0] == 0x1f && self.downloaded_data.bytes.as_ref().unwrap().inner.get_ref()[1] == 0x8b;
                    if has_gz_stream {
                        self.gz_decoder = Some(Decoder::new(self.downloaded_data.bytes.take().unwrap()));
                    }
                }
            };
        }
        Ok(is_last_chunk)
    }

    fn reader_mut(&mut self) -> &mut ResumableReader {
        if let Some(mut gz_decoder) = self.gz_decoder.as_mut() {
            gz_decoder.as_inner_mut()
        } else {
            self.downloaded_data.bytes.as_mut().unwrap()
        }
    }

    fn read_from_stream(&self) -> js_sys::Promise {
        match &self.reader {
            Some(stream_reader) => {
                stream_reader.read().unwrap()
            }
            None => {
                panic!("No stream reader defined")
            }
        }
    }

    fn decoded_bytes(&self) -> &[u8] {
        &self.downloaded_data.decoded[0..self.downloaded_data.num_decompressed_bytes]
    }

    fn pump_gz(&mut self) -> io::Result<usize> {
        // See if we need to reallocate our decoded buffer:
        let pump_size = (160 * 120 * 2 * 2) as usize; // Approx 2 frames worth
        if self.downloaded_data.num_decompressed_bytes as isize > self.downloaded_data.decoded.len() as isize - pump_size as isize {
            // Reallocate when we're 1KB from the end of the buffer:
            self.downloaded_data.decoded.append(&mut vec![0u8;pump_size]);
        }
        self.gz_decoder.as_mut().unwrap().read(&mut self.downloaded_data.decoded[self.downloaded_data.num_decompressed_bytes..])
    }

    #[wasm_bindgen(js_name = initWithReadableStream)]
    pub fn init_with_readable_stream(&mut self, stream: ReadableStreamDefaultReader, size: f64) -> Result<JsValue, JsValue> {
        self.reader = Some(stream);
        self.downloaded_data = DownloadedData::new();
        self.downloaded_data.bytes = Some(ResumableReader::new_with_capacity(size as usize));
        self.gz_decoder = None;
        self.frame_buffer = CptvFrame::new();
        self.clip_info = CptvHeader::UNINITIALISED;
        self.playback_info = PlaybackInfo::new();

        // TODO(jon): Should do an initial Pump?

        // We probably want to store this reader object too.
        Ok(JsValue::from_bool(true))
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

                // We need `input` to be a readable stream, does such a thing exist in wasm world?

                let mut gz_decoder = Decoder::new(&input[..]);
                let bytes_read = match gz_decoder.read_to_end(&mut decoded) {
                    Ok(r) => r,
                    Err(e) => {
                        info!("{:?}", e);
                        0
                    }
                };
                info!("gzip read {} bytes", bytes_read);

                // TODO(jon): It would be good if decoding wasn't eager, but on demand.
                // If the user calls getNextFrame(), that's what drives the decode.
                // If the user calls getHeader(), get just enough to decode the header.
                decoded
            } else {
                input.to_vec()
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
                        #[cfg(feature = "cptv3-support")]
                        {
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
                        #[cfg(not(feature = "cptv3-support"))]
                        {
                            panic!("cptv3 support not enabled")
                        }
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
    pub fn get_frame(&mut self, number: u32, image_data: &mut [u8]) -> bool {
        match self.clip_info {
            CptvHeader::V2(_) => {
                #[cfg(feature = "cptv2-support")]
                {
                    info!("Get frame {}", number);
                    get_frame_v2(self, number, image_data)
                }
                #[cfg(not(feature = "cptv2-support"))]
                {
                    panic!("cptv2 support not enabled")
                }
            },
            CptvHeader::V3(_) => {
                #[cfg(feature = "cptv3-support")]
                {
                    get_frame_v3(self, number, image_data)
                }
                #[cfg(not(feature = "cptv3-support"))]
                {
                    panic!("cptv3 support not enabled")
                }
            },
            CptvHeader::UNINITIALISED => false,
        }
    }

    #[wasm_bindgen(js_name = streamComplete)]
    pub fn stream_complete(&self) -> bool {
        self.downloaded_data.stream_ended && self.downloaded_data.gzip_ended
    }

    pub fn try_goto_loaded_frame(&mut self, n: usize) -> bool {
        match self.iframes.get(self.get_frame_index(n)) {
            None => self.stream_complete(),
            Some(_) => true
        }
        // // If the frame is already loaded in our cache, just set it to be the current frame.
        // let frame_num = if let Some(first_frame) = self.iframes.get(0) {
        //     if first_frame.is_background_frame {
        //         frame_num + 1
        //     } else {
        //         frame_num
        //     }
        // } else {
        //     frame_num
        // };
        // if let Some(frame) = self.iframes.get(frame_num) {
        //     self.frame_buffer = (*frame).clone();
        //     self.playback_info.prev_frame = frame_num;
        //     true
        // } else {
        //     if self.stream_complete() {
        //         info!("Getting last frame {}", self.iframes.len() - 1);
        //         if let Some(total) = self.playback_info.total_frames {
        //             assert!(self.playback_info.prev_frame <= total);
        //             let frame_num = usize::max(total, self.iframes.len() - 1);
        //             // TODO(jon): We can probably get rid of this cloning, just reference the current frame index
        //             self.frame_buffer = self.iframes.last().unwrap().clone();
        //             self.playback_info.prev_frame = frame_num;
        //             assert!(self.at_end_of_video());
        //         } else {
        //             panic!("Shouldn't get here");
        //         }
        //         true
        //     } else {
        //         false
        //     }
        // }
    }

    #[wasm_bindgen(js_name = seekToFrame)]
    pub async fn seek_to_frame(mut context: CptvPlayerContext, frame_num: usize) -> Result<CptvPlayerContext, JsValue> {
        while !context.try_goto_loaded_frame(frame_num) {
            // Load until we have the frame.
            context = CptvPlayerContext::fetch_raw_frame(context).await?;
        }
        Ok(context)
    }

    #[wasm_bindgen(js_name = fetchRawFrame)]
    pub async fn fetch_raw_frame(mut context: CptvPlayerContext) -> Result<CptvPlayerContext, JsValue> {
        loop {
            match context.downloaded_data.frame_data() {
                Some(frame_data) => {
                    // Try to parse a frame header:
                    let width = context.get_width() as usize;
                    let height = context.get_height() as usize;
                    let current_frame_offset = context.downloaded_data.latest_frame_offset.unwrap();
                    let frame_data_from_latest_offset = &frame_data[current_frame_offset..];
                    let initial_length = frame_data_from_latest_offset.len();
                    match decode_frame_header_v2(frame_data_from_latest_offset, width, height) {
                        Ok((remaining, (frame_data, mut frame))) => {
                            //info!("Got frame {:?}", frame);

                            //
                            //context.frame_buffer.image_data = FrameData::with_dimensions(context.get_width() as usize, context.get_height() as usize);
                            unpack_frame_v2(context.iframes.last(), frame_data, &mut frame);

                            //if !frame.is_background_frame {
                            let min = frame.image_data.min();
                            let within_ffc_timeout = match frame.last_ffc_time {
                                Some(last_ffc_time) => {
                                    (frame.time_on as i32 - last_ffc_time as i32) < 5000
                                }
                                None => false
                            };
                            if min != 0 && (frame.is_background_frame || !within_ffc_timeout) { // If the minimum value is zero, it's probably a glitched frame.
                                context.playback_info.min_value = u16::min(context.playback_info.min_value, frame.image_data.min());
                                context.playback_info.max_value = u16::max(context.playback_info.max_value, frame.image_data.max());
                            }
                            // TODO(jon): Should we write the offset of each frame header into some TOC structure for easy later lookup,
                            // so that we don't have to scan through all the frames from the start to find a given frame?

                            context.downloaded_data.latest_frame_offset = Some(current_frame_offset + (initial_length - remaining.len()));
                            context.iframes.push(frame);

                            if context.frame_buffer.is_background_frame {
                                // Skip the background frame
                                continue;
                            }
                            break;
                        }
                        Err(e) => {
                            match e {
                                nom::Err::Incomplete(e) => {
                                    // Loop again and fetch more bytes.

                                    //info!("Err incomplete {:?}", e);
                                    if context.stream_complete() {
                                        // We're trying to read past the available frames.
                                        // Now we know how many frames there actually were in the video,
                                        // and can set that information.
                                        context.playback_info.total_frames = Some(context.iframes.len());
                                        info!("Stream completed with total frames {:?} (including any background frame)", context.playback_info.total_frames.unwrap());
                                        break;
                                    }
                                    context = CptvPlayerContext::fetch_bytes(context).await?.0;
                                },
                                nom::Err::Error((i, kind)) | nom::Err::Failure((i, kind)) => {
                                    // We might have some kind of parsing error with the header?
                                    info!("{}", &format!("kind {:?}", kind));
                                    break;
                                }
                            }
                        }
                    }
                },
                None => {
                    // Try to get to the end of the header:
                    context = CptvPlayerContext::fetch_header(context).await?;
                }
            }

            // See if the next frame already exists in our buffer?  Actually, maybe we don't want to store
            // each decoded frame?  Although, they'll be similar in size to the ungzipped versions?

            // We should be able to seek through all decoded frames quickly, and then if we keep "key frames"
            // in a buffer, we can decode reasonably quickly to a given frame.
        }
        Ok(context)
    }

    #[wasm_bindgen(js_name = totalFrames)]
    pub fn get_total_frames(&self) -> usize {
        self.iframes.len()
    }

    // #[wasm_bindgen(js_name = atEndOfVideo)]
    // pub fn at_end_of_video(&self) -> bool {
    //     match self.playback_info.total_frames {
    //         Some(total) => self.iframes.len() == total,
    //         None => false
    //     }
    // }

    #[wasm_bindgen(js_name = bytesLoaded)]
    pub fn get_bytes_loaded(&mut self) -> usize {
        self.reader_mut().available
    }

    #[wasm_bindgen(js_name = getFrameHeader)]
    pub fn get_frame_header_n(&self, n: usize) -> JsValue {

        // TODO(jon):
        // Get frame header for frame n.
        // If frame n hasn't downloaded yet, return false.

        // We don't keep an internal "current frame" state in the wasm, we just keep track of
        // how much has decoded etc.
        // Playback info should be handled in the JS layer.
        match self.iframes.get(self.get_frame_index(n)) {
            Some(frame) => serde_wasm_bindgen::to_value(frame).unwrap(),
            None => JsValue::null()
        }
    }

    fn get_frame_index(&self, n: usize) -> usize {
        let has_background_frame = match &self.clip_info {
            CptvHeader::V2(h) => match h.has_background_frame {
                Some(bg_frame) => bg_frame,
                None => false
            },
            _ => false,
        };
        if has_background_frame {
            n + 1
        } else {
            n
        }
    }

    // TODO(jon): Not sure that this does the right thing?
    #[wasm_bindgen(js_name = getRawFrameN)]
    pub fn get_raw_frame_n(&self, n: usize) -> Uint16Array {
        // Get the raw frame specified by a frame number
        // If frame n hasn't yet downloaded, return an empty array.
        match self.iframes.get(self.get_frame_index(n)) {
            Some(frame) => unsafe { Uint16Array::view(frame.image_data.data()) },
            None => Uint16Array::new_with_length(0)
        }
    }

    #[wasm_bindgen(js_name = getBackgroundFrame)]
    pub fn get_background_frame(&self) -> Uint16Array {
        let has_background_frame = match &self.clip_info {
            CptvHeader::V2(h) => match h.has_background_frame {
                Some(bg_frame) => bg_frame,
                None => false
            },
            _ => false,
        };
        if has_background_frame {
            match self.iframes.get(0) {
                Some(frame) => unsafe { Uint16Array::view(frame.image_data.data()) },
                None => Uint16Array::new_with_length(0)
            }
        } else {
            Uint16Array::new_with_length(0)
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
        self.playback_info.min_value
    }

    #[wasm_bindgen(js_name = getMaxValue)]
    pub fn get_max_value(&self) -> u16 {
        self.playback_info.max_value
    }

    async fn fetch_bytes(mut context: CptvPlayerContext) -> Result<(CptvPlayerContext, bool), JsValue> {
        if context.reader_mut().used < context.reader_mut().available {
            let bytes_read = match context.pump_gz() {
                Ok(r) => r,
                Err(e) => {
                    if e.kind() == ErrorKind::UnexpectedEof {
                        context.downloaded_data.gzip_ended = true;
                    }
                    if !context.downloaded_data.stream_ended {
                        let is_last_chunk = context.get_bytes().await?;
                        if is_last_chunk {
                            context.reader_mut().stream_ended = true;
                            context.downloaded_data.stream_ended = true;
                        }
                    } else {
                        info!("Stream ended");
                    }
                    return Ok((context, true));
                }
            };
            context.downloaded_data.num_decompressed_bytes += bytes_read;
        } else {
            info!("We've used all available bytes here, and should ask for more before continuing");
            //let (num_bytes_read, is_last_chunk) = context.get_bytes().await?;
        }
        Ok((context, false))
    }

    #[wasm_bindgen(js_name = fetchHeader)]
    pub async fn fetch_header(mut context: CptvPlayerContext) -> Result<CptvPlayerContext, JsValue> {
        // If there's not enough data in the buffer to get the header, pump here.
        // Read some initial bytes in from the network if there aren't enough?

        // Do we want to do the initial pump here or on our init function?
        context.get_bytes().await?;
        // First we need to decode the gzipped contents into our buffer.
        if context.gz_decoder.is_some() {
            loop {
                // TODO(jon): Make sure we've decoded all the bytes we already had
                let (ctx, should_continue) = CptvPlayerContext::fetch_bytes(context).await?;
                context = ctx;
                if should_continue {
                    continue;
                }
                let input = context.decoded_bytes();
                let initial_len = input.len();
                match decode_cptv_header(input) {
                    Ok((remaining, header)) => {
                        context.downloaded_data.first_frame_offset = Some(initial_len - remaining.len());
                        context.downloaded_data.latest_frame_offset = Some(0);
                        context.clip_info = header;
                        // Now we can initialise the previous frame buffer
                        //context.frame_buffer.image_data = FrameData::with_dimensions(context.get_width() as usize, context.get_height() as usize);
                        break;
                    }
                    Err(e) => {
                        match e {
                            nom::Err::Incomplete(_) => {
                                // Loop again and fetch more bytes.
                                continue;
                            },
                            nom::Err::Error((i, kind)) => {
                                // We might have some kind of parsing error with the header?
                                info!("{}", &format!("kind {:?}", kind));
                                break;

                            }
                            _ => {
                                info!("Unknown error");
                                break;
                            }
                        }
                    }
                }
            }
        } else {
            unimplemented!("We're only dealing with cptv2 gzipped streams at the moment")
        }
        Ok(context)
    }

    #[wasm_bindgen(js_name = getHeader)]
    pub fn get_header(&self) -> JsValue {
        match &self.clip_info {
            //CptvHeader::V2(h) => h.clone(),
            CptvHeader::V2(h) => serde_wasm_bindgen::to_value(&h).unwrap(),
            _ => panic!("failed to parse header")//JsValue::from_str("Unable to parse header"),
        }
    }

    #[wasm_bindgen(js_name = initBufferWithSize)]
    pub fn init_buffer_with_size(&mut self, size: usize) -> Result<(), JsValue> {
        self.downloaded_data.bytes = Some(ResumableReader::new_with_capacity(size));
        Ok(())
    }
}
