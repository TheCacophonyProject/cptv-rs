use derivative::Derivative;
use std::fmt;
use std::fmt::{Error, Formatter};
use std::ops::{Index, IndexMut};

#[derive(Debug)]
pub struct CptvHeader {
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub compression: u8,
    pub device_name: String,

    pub motion_config: Option<String>,
    pub preview_secs: Option<u8>,
    pub latitude: Option<f32>,
    pub longitude: Option<f32>,
    pub loc_timestamp: Option<u64>,
    pub altitude: Option<f32>,
    pub accuracy: Option<f32>,
}

#[derive(Clone, Copy)]
pub struct FrameData([[i16; 160]; 120]);

impl FrameData {
    pub fn empty() -> FrameData {
        FrameData([[0i16; 160]; 120])
    }

    pub fn width(&self) -> usize {
        self[0].len()
    }

    pub fn height(&self) -> usize {
        self.0.len()
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                &self[0] as *const i16 as *const u8,
                std::mem::size_of_val(self),
            )
        }
    }
}

impl Index<usize> for FrameData {
    type Output = [i16; 160];

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for FrameData {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct CptvFrame {
    pub time_on: u32,
    pub bit_width: u8,
    pub frame_size: u32,
    pub last_ffc_time: u32,
    #[derivative(Debug = "ignore")]
    pub image_data: FrameData,
}

pub struct Cptv {
    pub meta: CptvHeader,
    pub frames: Vec<CptvFrame>,
}

impl fmt::Debug for Cptv {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "CPTV file with {} frames", self.frames.len())
    }
}

pub struct FrameHeader {
    length: u32,
    time_on: u32,
    last_ffc_time: u32,
    pixel_size: u8,
}

impl FrameHeader {
    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const FrameHeader as *const u8,
                std::mem::size_of_val(self),
            )
        }
    }
}

#[repr(C)]
#[derive(PartialEq, Debug)]
pub enum FieldType {
    Timestamp = b'T' as isize,
    Width = b'X' as isize,
    Height = b'Y' as isize,
    Compression = b'C' as isize,
    DeviceName = b'D' as isize,
    MotionConfig = b'M' as isize,
    PreviewSecs = b'P' as isize,
    Latitude = b'L' as isize,
    Longitude = b'O' as isize,
    LocTimestamp = b'S' as isize,
    Altitude = b'A' as isize,
    Accuracy = b'U' as isize,
    MinValue = b'V' as isize,
    MaxValue = b'H' as isize,
    TableOfContents = b'Q' as isize,
    NumFrames = b'N' as isize,
    FrameRate = b'R' as isize,
    FramesPerIframe = b'I' as isize,
    FrameHeader = b'F' as isize,
    PixelBytes = b'w' as isize,
    FrameSize = b'f' as isize,
    LastFfcTime = b'c' as isize,
    TimeOn = b't' as isize,
}

/// Unused

// Need to serialise this:
struct ClipHeader {
    magic_bytes: [u8; 4],
    version: u8,
    timestamp: u64, // At time of recording start?
    width: u32,
    height: u32,
    compression: u8, // None, zlib. zstd?
    device_name: String,

    motion_config: Option<String>,
    preview_secs: Option<u8>,
    latitude: Option<f32>,
    longitude: Option<f32>,
    loc_timestamp: Option<u64>,
    altitude: Option<f32>,
    accuracy: Option<f32>,

    // Used to get dynamic range of clip for normalisation at runtime:
    min_val: u16,
    max_val: u16,
}

struct ClipToc {
    num_frames: u32,
    frames_per_iframe: u32,
    fps: u8,
    length: u32,
    // length x u32 offsets into the compressed stream.
}

impl ClipHeader {
    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const ClipHeader as *const u8,
                std::mem::size_of_val(self),
            )
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        Vec::new()
    }
}
