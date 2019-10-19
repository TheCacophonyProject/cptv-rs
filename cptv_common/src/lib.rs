//use derivative::Derivative;
//use std::fmt;
//use std::fmt::{Error, Formatter};
use std::ops::{Index, IndexMut};

//#[derive(Debug)]
pub struct Cptv2Header {
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

//#[derive(Debug)]
pub struct Cptv3Header {
    pub v2: Cptv2Header,
    pub min_value: u16,
    pub max_value: u16,
    pub toc: Vec<u32>,
    pub num_frames: u32,
    pub frame_rate: u8,
    pub frames_per_iframe: u8,
}

impl Cptv3Header {
    pub fn new() -> Cptv3Header {
        Cptv3Header {
            v2: Cptv2Header {
                timestamp: 0,
                width: 0,
                height: 0,
                compression: 0,
                device_name: "".to_string(),
                motion_config: None,
                preview_secs: None,
                latitude: None,
                longitude: None,
                loc_timestamp: None,
                altitude: None,
                accuracy: None,
            },
            min_value: 0,
            max_value: 0,
            toc: Vec::new(),
            num_frames: 0,
            frame_rate: 0,
            frames_per_iframe: 0,
        }
    }
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

//#[derive(Derivative)]
//#[derivative(Debug, Copy, Clone)]
pub struct CptvFrame {
    pub time_on: u32,
    pub bit_width: u8,
    pub frame_size: u32,
    pub last_ffc_time: u32,
    //#[derivative(Debug = "ignore")]
    pub image_data: FrameData,
}

impl CptvFrame {
    pub fn new() -> CptvFrame {
        CptvFrame {
            time_on: 0,
            bit_width: 0,
            frame_size: 0,
            last_ffc_time: 0,
            image_data: FrameData::empty(),
        }
    }
}

pub struct Cptv2 {
    pub meta: Cptv2Header,
    pub frames: Vec<CptvFrame>,
}

pub struct Cptv3 {
    pub meta: Cptv3Header,
    pub frames: Vec<CptvFrame>,
}

//impl fmt::Debug for Cptv2 {
//    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
//        write!(f, "CPTV file with {} frames", self.frames.len())
//    }
//}

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

#[repr(u8)]
#[derive(PartialEq)]
pub enum FieldType {
    Header = b'H',
    Timestamp = b'T',
    Width = b'X',
    Height = b'Y',
    Compression = b'C',
    DeviceName = b'D',
    MotionConfig = b'M',
    PreviewSecs = b'P',
    Latitude = b'L',
    Longitude = b'O',
    LocTimestamp = b'S',
    Altitude = b'A',
    Accuracy = b'U',

    MinValue = b'V',
    MaxValue = b'W',
    TableOfContents = b'Q',
    NumFrames = b'N',
    FrameRate = b'R',
    FramesPerIframe = b'I',
    FrameHeader = b'F',

    PixelBytes = b'w',
    FrameSize = b'f',
    LastFfcTime = b'c',
    TimeOn = b't',
    Unknown = b';',
}

impl From<u8> for FieldType {
    fn from(val: u8) -> Self {
        use FieldType::*;
        match val {
            b'H' => Header,
            b'T' => Timestamp,
            b'X' => Width,
            b'Y' => Height,
            b'C' => Compression,
            b'D' => DeviceName,
            b'M' => MotionConfig,
            b'P' => PreviewSecs,
            b'L' => Latitude,
            b'O' => Longitude,
            b'S' => LocTimestamp,
            b'A' => Altitude,
            b'U' => Accuracy,
            b'V' => MinValue,
            b'W' => MaxValue,
            b'Q' => TableOfContents,
            b'N' => NumFrames,
            b'R' => FrameRate,
            b'I' => FramesPerIframe,
            b'F' => FrameHeader,
            b'w' => PixelBytes,
            b'f' => FrameSize,
            b'c' => LastFfcTime,
            b't' => TimeOn,
            _ => Unknown,
        }
    }
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

#[inline(always)]
fn average_2(a: i16, b: i16) -> i16 {
    (a + b) / 2
}

pub fn predict_left(data: &FrameData, x: usize, y: usize) -> i16 {
    let left = if x == 0 {
        if y == 0 {
            0
        } else {
            data[y - 1][x]
        }
    } else {
        data[y][x - 1]
    };
    let top = if y == 0 { 0 } else { data[y - 1][x] };
    let top_left = if y == 0 || x == 0 {
        left
    } else {
        data[y - 1][x - 1]
    };
    let top_right = if x == data.width() - 1 || y == 0 {
        top
    } else {
        data[y - 1][x + 1]
    };
    average_2(average_2(left, top_left), average_2(top, top_right))
}

pub fn predict_right(data: &FrameData, x: usize, y: usize) -> i16 {
    let right = if x == data.width() - 1 {
        if y == 0 {
            0
        } else {
            data[y - 1][x]
        }
    } else {
        data[y][x + 1]
    };
    let top = if y == 0 { 0 } else { data[y - 1][x] };
    let top_left = if y == 0 || x == 0 {
        right
    } else {
        data[y - 1][x - 1]
    };
    let top_right = if x == data.width() - 1 || y == 0 {
        top
    } else {
        data[y - 1][x + 1]
    };
    average_2(average_2(right, top_left), average_2(top, top_right))
}
