use chrono::NaiveDateTime;
use core::fmt;
#[allow(unused)]
use log::{info, trace, warn};
use serde::Serialize;
use std::fmt::{Debug, Formatter};
use std::ops::{Index, IndexMut};
use std::time::Duration;

#[derive(Serialize)]
pub struct Cptv2Header {
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub compression: u8,
    #[serde(rename = "deviceName")]
    pub device_name: String,

    pub fps: Option<u8>,
    pub brand: Option<String>,
    pub model: Option<String>,
    #[serde(rename = "deviceId")]
    pub device_id: Option<u32>,
    #[serde(rename = "serialNumber")]
    pub serial_number: Option<u32>,
    #[serde(rename = "firmwareVersion")]
    pub firmware_version: Option<String>,
    #[serde(rename = "motionConfig")]
    pub motion_config: Option<String>,
    #[serde(rename = "previewSecs")]
    pub preview_secs: Option<u8>,
    pub latitude: Option<f32>,
    pub longitude: Option<f32>,
    #[serde(rename = "locTimestamp")]
    pub loc_timestamp: Option<u64>,
    pub altitude: Option<f32>,
    pub accuracy: Option<f32>,
    #[serde(rename = "hasBackgroundFrame")]
    pub has_background_frame: Option<bool>,
}

impl Cptv2Header {
    pub fn new() -> Cptv2Header {
        Cptv2Header {
            timestamp: 0,
            width: 0,
            height: 0,
            compression: 0,
            device_name: "".to_string(),
            fps: None,
            brand: None,
            model: None,
            device_id: None,
            serial_number: None,
            firmware_version: None,
            motion_config: None,
            preview_secs: None,
            latitude: None,
            longitude: None,
            loc_timestamp: None,
            altitude: None,
            accuracy: None,
            has_background_frame: None,
        }
    }
}

impl Debug for Cptv2Header {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cptv2Header")
            .field(
                "timestamp",
                &NaiveDateTime::from_timestamp((self.timestamp as f64 / 1000000.0) as i64, 0),
            )
            .field("width", &self.width)
            .field("height", &self.height)
            .field("compression", &self.compression)
            .field("device_name", &self.device_name)
            .field("fps", &self.fps)
            .field("brand", &self.brand)
            .field("model", &self.model)
            .field("device_id", &self.device_id)
            .field("serial_number", &self.serial_number)
            .field("firmware_version", &self.firmware_version)
            .field(
                "motion_config",
                &self.motion_config.as_ref().unwrap_or(&String::from("None")),
            )
            .field("preview_secs", &self.preview_secs)
            .field("latitude", &self.latitude)
            .field("longitude", &self.longitude)
            .field("loc_timestamp", &self.loc_timestamp)
            .field("altitude", &self.altitude)
            .field("accuracy", &self.accuracy)
            .field("has_background_frame", &self.has_background_frame)
            .finish()
    }
}

// Cptv3 header includes the v2 header + additional fields to allow seeking.
#[derive(Debug)]
pub struct Cptv3Header {
    pub v2: Cptv2Header,
    pub min_value: u16,
    pub max_value: u16,
    pub toc: Vec<u32>,
    pub num_frames: u32,
    pub frames_per_iframe: u8,
}

impl Cptv3Header {
    pub fn new() -> Cptv3Header {
        Cptv3Header {
            v2: Cptv2Header::new(),
            min_value: 0,
            max_value: 0,
            toc: Vec::new(),
            num_frames: 0,
            frames_per_iframe: 0,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct FrameData {
    #[serde(skip_serializing)]
    data: Vec<u16>,
    width: usize,
    height: usize,
    min: u16,
    max: u16,
}

impl FrameData {
    pub fn with_dimensions(width: usize, height: usize) -> FrameData {
        FrameData {
            data: vec![0; width * height],
            width,
            height,
            min: u16::MAX,
            max: u16::MIN,
        }
    }

    pub fn min(&self) -> u16 {
        self.min
    }

    pub fn max(&self) -> u16 {
        self.max
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn data(&self) -> &[u16] {
        &self.data
    }

    pub fn set(&mut self, x: usize, y: usize, val: u16) {
        // Ignore edge pixels for this?

        self.max = u16::max(self.max, val);
        self.min = u16::min(self.min, val);
        self[y][x] = val;
    }

    // This was a function made for fixing up our "black pixel" syncing offset issues
    pub fn offset(&self, offset: usize) -> FrameData {
        let mut frame = FrameData::with_dimensions(self.width, self.height);
        let mut pixels = self.data.iter().skip(offset);
        for y in 0..frame.height() {
            for x in 0..frame.width() {
                let pixel = *pixels.next().unwrap_or(&0u16);
                //assert!(pixel >= 0);
                frame[y][x] = pixel;
            }
        }
        frame
    }
}

// Gives the row
impl Index<usize> for FrameData {
    type Output = [u16];

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[(index * self.width)..(index * self.width) + self.width]
    }
}

impl IndexMut<usize> for FrameData {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[(index * self.width)..(index * self.width) + self.width]
    }
}

#[derive(Serialize, Clone)]
pub struct CptvFrame {
    #[serde(rename = "timeOnMs")]
    pub time_on: u32,

    // Is bit_width needed?  Is frame_size
    #[serde(skip_serializing)]
    pub bit_width: u8,
    #[serde(skip_serializing)]
    pub frame_size: u32,

    // Some cameras may not have FFC information, so this is optional.
    #[serde(rename = "lastFfcTimeMs")]
    pub last_ffc_time: Option<u32>,
    #[serde(rename = "lastFfcTempC")]
    pub last_ffc_temp_c: Option<f32>,
    #[serde(rename = "frameTempC")]
    pub frame_temp_c: Option<f32>,

    #[serde(rename = "isBackgroundFrame")]
    pub is_background_frame: bool,

    // Raw image data?
    #[serde(rename = "imageData")]
    pub image_data: FrameData,
}

impl CptvFrame {
    pub fn new() -> CptvFrame {
        CptvFrame {
            time_on: 0,
            bit_width: 0,
            frame_size: 0,
            last_ffc_time: None,
            last_ffc_temp_c: None,
            frame_temp_c: None,
            is_background_frame: false,
            image_data: FrameData::with_dimensions(0, 0),
        }
    }
}

impl Debug for CptvFrame {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CptvFrame")
            .field(
                "last_ffc_time",
                // To get absolute time, need recording start time from header:
                &match self.last_ffc_time {
                    Some(timestamp) => format!(
                        "{:?}s ago",
                        &Duration::from_millis(self.time_on as u64 - timestamp as u64).as_secs()
                    ),
                    None => "None".to_string(),
                },
            )
            .field("time_on", &{
                let seconds = Duration::from_millis(self.time_on as u64).as_secs();
                let minutes = seconds / 60;
                let hours = minutes / 60;
                let minutes = minutes - (hours * 60);
                let seconds = seconds - ((hours * 60 * 60) + (minutes * 60));
                if hours > 0 {
                    // Minutes
                    format!("{}h, {}m, {}s", hours, minutes, seconds)
                } else if minutes > 0 {
                    format!("{}m, {}s", minutes, seconds)
                } else {
                    format!("{}s", seconds)
                }
            })
            .field("frame_temp_c", &self.frame_temp_c)
            .field("last_ffc_temp_c", &self.last_ffc_temp_c)
            .field("bit_width", &self.bit_width)
            .field("is_background_frame", &self.is_background_frame)
            .field(
                "image_data",
                &format!(
                    "FrameData({}x{})",
                    &self.image_data.width, &self.image_data.height
                ),
            )
            .finish()
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

#[repr(u8)]
#[derive(PartialEq, Debug)]
pub enum FieldType {
    // K remaining
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
    Model = b'E',
    Brand = b'B',
    DeviceID = b'I',
    FirmwareVersion = b'V',
    CameraSerial = b'N',
    FrameRate = b'Z',
    BackgroundFrame = b'g',

    // TODO: Other header fields I've added to V2
    MinValue = b'R',
    MaxValue = b'W',
    TableOfContents = b'Q',
    NumFrames = b'J',
    FramesPerIframe = b'G',
    FrameHeader = b'F',

    PixelBytes = b'w',
    FrameSize = b'f',
    LastFfcTime = b'c',
    FrameTempC = b'a',
    LastFfcTempC = b'b',
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
            b'E' => Model,
            b'B' => Brand,
            b'I' => DeviceID,
            b'M' => MotionConfig,
            b'P' => PreviewSecs,
            b'L' => Latitude,
            b'O' => Longitude,
            b'S' => LocTimestamp,
            b'A' => Altitude,
            b'U' => Accuracy,
            b'R' => MinValue,
            b'W' => MaxValue,
            b'N' => CameraSerial,
            b'V' => FirmwareVersion,
            b'Q' => TableOfContents,
            b'J' => NumFrames,
            b'Z' => FrameRate,
            b'G' => FramesPerIframe,
            b'F' => FrameHeader,
            b'g' => BackgroundFrame,
            b'w' => PixelBytes,
            b'f' => FrameSize,
            b'c' => LastFfcTime,
            b't' => TimeOn,
            b'a' => FrameTempC,
            b'b' => LastFfcTempC,
            _ => Unknown,
        }
    }
}

#[inline(always)]
fn average_2(a: i32, b: i32) -> i16 {
    ((a + b) / 2) as i16
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
    average_2(
        average_2(left as i32, top_left as i32) as i32,
        average_2(top as i32, top_right as i32) as i32,
    )
    //0
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
    average_2(
        average_2(right as i32, top_left as i32) as i32,
        average_2(top as i32, top_right as i32) as i32,
    )
    //0
}
