use cptv_common::{
    predict_left, predict_right, Cptv2Header, Cptv3Header, CptvFrame, FieldType, FrameData,
};
#[allow(unused)]
use log::{info, trace, warn};
use nom::bytes::complete::{tag, take};
use nom::number::complete::{le_f32, le_i8, le_u16, le_u32, le_u64, le_u8};
use std::ops::RangeInclusive;

#[derive(Debug)]
pub enum CptvHeader {
    UNINITIALISED,
    V3(Cptv3Header),
    V2(Cptv2Header),
}

fn decode_cptv3_header(i: &[u8]) -> nom::IResult<&[u8], CptvHeader> {
    let mut meta = Cptv3Header::new();
    let (i, _) = tag(b"H")(i)?;
    let (i, _header_field_len_size) = le_u8(i)?;
    let (i, num_header_fields) = le_u8(i)?;
    let mut outer = i;
    for _ in 0..num_header_fields {
        let (i, field) = le_u8(outer)?;
        let (i, field_length) = le_u8(i)?;
        let (i, val) = take(field_length)(i)?;
        outer = i;
        let field_type = FieldType::from(field);
        match field_type {
            FieldType::Timestamp => {
                meta.v2.timestamp = le_u64(val)?.1;
            }
            FieldType::Width => {
                meta.v2.width = le_u32(val)?.1;
            }
            FieldType::Height => {
                meta.v2.height = le_u32(val)?.1;
            }
            FieldType::Compression => {
                meta.v2.compression = le_u8(val)?.1;
            }
            FieldType::DeviceName => {
                meta.v2.device_name = String::from_utf8_lossy(val).into();
            }

            // Optional fields
            FieldType::MotionConfig => {
                meta.v2.motion_config = Some(String::from_utf8_lossy(val).into());
            }
            FieldType::PreviewSecs => {
                meta.v2.preview_secs = Some(le_u8(val)?.1);
            }
            FieldType::Latitude => {
                meta.v2.latitude = Some(le_f32(val)?.1);
            }
            FieldType::Longitude => {
                meta.v2.longitude = Some(le_f32(val)?.1);
            }
            FieldType::LocTimestamp => {
                meta.v2.loc_timestamp = Some(le_u64(val)?.1);
            }
            FieldType::Altitude => {
                meta.v2.altitude = Some(le_f32(i)?.1);
            }
            FieldType::Accuracy => {
                meta.v2.accuracy = Some(le_f32(val)?.1);
            }
            // V3 fields
            FieldType::MinValue => {
                meta.min_value = le_u16(val)?.1;
            }
            FieldType::MaxValue => {
                meta.max_value = le_u16(val)?.1;
            }
            FieldType::NumFrames => {
                meta.num_frames = le_u32(val)?.1;
            }
            FieldType::FrameRate => {
                meta.frame_rate = le_u8(val)?.1;
            }
            FieldType::FramesPerIframe => {
                meta.frames_per_iframe = le_u8(val)?.1;
            }
            _ => {
                //panic!("Unknown header field type {}", field)
                std::process::abort();
            }
        }
    }
    let (i, field) = le_u8(outer)?;
    assert_eq!(field, b'Q');
    // This will always be the last block, and after this, frames start.
    let (i, num_iframes) = le_u32(i)?;
    outer = i;
    for _ in 0..num_iframes {
        let (a, offset) = le_u32(outer)?;
        meta.toc.push(offset);
        outer = a;
    }
    Ok((outer, CptvHeader::V3(meta)))
}

pub fn decode_cptv2_header(i: &[u8]) -> nom::IResult<&[u8], CptvHeader> {
    let mut meta = Cptv2Header::new();
    let (i, _) = tag(b"H")(i)?;
    let (i, num_header_fields) = le_u8(i)?;
    //info!("num header fields {}", num_header_fields);
    let mut outer = i;
    for _ in 0..num_header_fields {
        // TODO(jon): Fix order of this in cptv3 version
        let (i, field_length) = le_u8(outer)?;
        let (i, field) = le_u8(i)?;
        let (i, val) = take(field_length)(i)?;
        outer = i;
        // info!("{}", field_length);
        // info!("{:?}", field as char);
        let field_type = FieldType::from(field);
        match field_type {
            FieldType::Timestamp => {
                meta.timestamp = le_u64(val)?.1;
            }
            FieldType::Width => {
                meta.width = le_u32(val)?.1;
            }
            FieldType::Height => {
                meta.height = le_u32(val)?.1;
            }
            FieldType::Compression => {
                meta.compression = le_u8(val)?.1;
            }
            FieldType::DeviceName => {
                meta.device_name = String::from_utf8_lossy(val).into();
            }

            // Optional fields
            FieldType::FrameRate => meta.fps = Some(le_u8(val)?.1),
            FieldType::CameraSerial => meta.serial_number = Some(le_u32(val)?.1),
            FieldType::FirmwareVersion => {
                meta.firmware_version = Some(String::from_utf8_lossy(val).into());
            }
            FieldType::Model => {
                meta.model = Some(String::from_utf8_lossy(val).into());
            }
            FieldType::Brand => {
                meta.brand = Some(String::from_utf8_lossy(val).into());
            }
            FieldType::DeviceID => {
                meta.device_id = Some(String::from_utf8_lossy(val).into());
            }
            FieldType::MotionConfig => {
                meta.motion_config = Some(String::from_utf8_lossy(val).into());
            }
            FieldType::PreviewSecs => {
                meta.preview_secs = Some(le_u8(val)?.1);
            }
            FieldType::Latitude => {
                meta.latitude = Some(le_f32(val)?.1);
            }
            FieldType::Longitude => {
                meta.longitude = Some(le_f32(val)?.1);
            }
            FieldType::LocTimestamp => {
                meta.loc_timestamp = Some(le_u64(val)?.1);
            }
            FieldType::Altitude => {
                meta.altitude = Some(le_f32(i)?.1);
            }
            FieldType::Accuracy => {
                meta.accuracy = Some(le_f32(val)?.1);
            }
            _ => {
                warn!(
                    "Unknown header field type {}, {}",
                    field as char, field_length
                );
                //std::process::abort();
            }
        }
    }
    Ok((outer, CptvHeader::V2(meta)))
}

pub fn decode_cptv_header(i: &[u8]) -> nom::IResult<&[u8], CptvHeader> {
    let (i, _) = tag(b"CPTV")(i)?;
    let (i, version) = le_u8(i)?;
    match version {
        2 => decode_cptv2_header(i),
        3 => decode_cptv3_header(i),
        _ => panic!("Unknown CPTV version {}", version),
    }
}

pub fn get_dynamic_range(frame: &FrameData) -> RangeInclusive<u16> {
    let mut frame_max = 0;
    let mut frame_min = std::u16::MAX;
    assert_eq!(frame.as_values().iter().count(), 160 * 120);
    for val in frame.as_values().iter()
    //.take(frame.width() * frame.height() - 36)
    // NOTE(jon): Offset
    {
        frame_max = u16::max(*val, frame_max);
        frame_min = u16::min(*val, frame_min);
    }
    frame_min..=frame_max
}

pub fn decode_frame_v2<'a>(
    prev_frame: &CptvFrame,
    data: &'a [u8],
) -> nom::IResult<&'a [u8], CptvFrame> {
    let (i, _) = tag(b"F")(data)?;
    let (i, num_frame_fields) = le_u8(i)?;
    let mut frame = CptvFrame {
        time_on: 0,
        bit_width: 0,
        frame_size: 0,
        last_ffc_time: 0,
        image_data: FrameData::empty(),
    };
    let mut outer = i;
    for _ in 0..num_frame_fields as usize {
        let (i, field_length) = le_u8(outer)?;
        let (i, field_code) = le_u8(i)?;
        let (i, val) = take(field_length)(i)?;
        outer = i;
        let fc = FieldType::from(field_code);

        match fc {
            FieldType::TimeOn => {
                frame.time_on = le_u32(val)?.1;
            }
            FieldType::PixelBytes => {
                frame.bit_width = le_u8(val)?.1;
            }
            FieldType::FrameSize => {
                frame.frame_size = le_u32(val)?.1;
            }
            FieldType::LastFfcTime => {
                frame.last_ffc_time = le_u32(val)?.1;
            }
            _ => {
                //std::process::abort();
                warn!("Unknown frame field type '{}'", field_code as char);
            }
        }
    }
    assert!(frame.frame_size > 0);
    let (i, data) = take(frame.frame_size as usize)(outer)?;
    unpack_frame_v2(prev_frame, data, &mut frame);
    Ok((i, frame))
}

pub fn decode_frame<'a>(
    prev_frame: &CptvFrame,
    data: &'a [u8],
    is_iframe: bool,
) -> nom::IResult<&'a [u8], CptvFrame> {
    let (i, _) = tag(b"F")(data)?;
    let (i, _code_len) = le_u8(i)?;
    let (i, num_frame_fields) = le_u8(i)?;
    let mut frame = CptvFrame {
        time_on: 0,
        bit_width: 0,
        frame_size: 0,
        last_ffc_time: 0,
        image_data: FrameData::empty(),
    };
    let mut outer = i;
    for _ in 0..num_frame_fields as usize {
        let (i, field_code) = le_u8(outer)?;
        let (i, field_length) = le_u8(i)?;
        let (i, val) = take(field_length)(i)?;
        outer = i;
        let fc = FieldType::from(field_code);
        match fc {
            FieldType::TimeOn => {
                frame.time_on = le_u32(val)?.1;
            }
            FieldType::PixelBytes => {
                frame.bit_width = le_u8(val)?.1;
            }
            FieldType::FrameSize => {
                frame.frame_size = le_u32(val)?.1;
            }
            FieldType::LastFfcTime => {
                frame.last_ffc_time = le_u32(val)?.1;
            }
            _ => {
                //std::process::abort();
                warn!("Unknown frame field type {}", field_code as char);
            }
        }
    }
    assert!(frame.frame_size > 0);
    let (i, _) = take(frame.frame_size as usize)(outer)?;
    //copy_frame_data(data, &mut frame)?;
    unpack_frame(prev_frame, &mut frame, is_iframe);
    Ok((i, frame))
}
//
// fn copy_frame_data<'a>(data: &'a [u8], frame: &mut CptvFrame) -> nom::IResult<&'a [u8], ()> {
//     let mut data = data;
//     if frame.bit_width == 1 {
//         for y in 0..frame.image_data.height() {
//             for x in 0..frame.image_data.width() {
//                 let (remaining, val) = le_i8(data)?;
//                 data = remaining;
//                 frame.image_data[y][x] = val as i16;
//             }
//         }
//     } else if frame.bit_width == 2 {
//         for y in 0..frame.image_data.height() {
//             for x in 0..frame.image_data.width() {
//                 let (remaining, val) = le_i16(data)?;
//                 data = remaining;
//                 frame.image_data[y][x] = val;
//             }
//         }
//     }
//     Ok((data, ()))
// }

struct BitUnpacker<'a> {
    input: &'a [u8],
    offset: usize,
    bit_width: u8,
    num_bits: u8,
    bits: u32,
}

impl<'a> BitUnpacker<'a> {
    pub fn new(input: &'a [u8], bit_width: u8) -> BitUnpacker {
        BitUnpacker {
            input,
            offset: 0,
            bit_width,
            num_bits: 0,
            bits: 0,
        }
    }
}

#[inline(always)]
fn twos_uncomp(v: u32, width: u8) -> i32 {
    if v & (1 << (width - 1)) as u32 == 0 {
        v as i32
    } else {
        -(((!v + 1) & ((1 << width as u32) - 1)) as i32)
    }
}

impl<'a> Iterator for BitUnpacker<'a> {
    type Item = i32;
    fn next(&mut self) -> Option<Self::Item> {
        while self.num_bits < self.bit_width {
            match self.input.get(self.offset) {
                Some(byte) => {
                    self.bits |= (*byte as u32) << ((24 - self.num_bits) as u8) as u32;
                    self.num_bits += 8;
                }
                None => return None,
            }
            self.offset += 1;
        }
        let out = twos_uncomp(self.bits >> (32 - self.bit_width) as u32, self.bit_width);
        self.bits = self.bits << self.bit_width as u32;
        self.num_bits -= self.bit_width;
        Some(out)
    }
}

fn decode_image_data(
    i: &[u8],
    mut current_px: i32,
    width: usize,
    height: usize,
    frame: &mut CptvFrame,
    prev_frame: &CptvFrame,
) {
    // Take the first 4 bytes as initial delta value
    let prev_px = prev_frame.image_data[0][0] as i32;
    // Seed the initial pixel value
    assert!(prev_px + current_px <= u16::MAX as i32);
    assert!(prev_px + current_px >= 0);
    frame.image_data[0][0] = (prev_px + current_px) as u16;
    for (index, delta) in BitUnpacker::new(i, frame.bit_width)
        .take((width * height) - 1)
        .enumerate()
    {
        let index = index + 1;
        let y = index / width;
        let x = index % width;
        let x = if y & 1 == 1 { width - x - 1 } else { x };
        current_px += delta;
        let prev_px = prev_frame.image_data[y][x] as i32;
        assert!(prev_px + current_px <= std::u16::MAX as i32);
        assert!(prev_px + current_px >= 0);
        frame.image_data[y][x] = (prev_px + current_px) as u16;
        assert!(y * width + x <= width * height);
    }
}

fn unpack_frame_v2(prev_frame: &CptvFrame, data: &[u8], frame: &mut CptvFrame) {
    let initial_px = {
        let mut accum: i32 = 0;
        accum |= (data[3] as i32) << 24;
        accum |= (data[2] as i32) << 16;
        accum |= (data[1] as i32) << 8;
        accum |= data[0] as i32;
        accum
    };
    decode_image_data(
        &data[4..],
        initial_px,
        160usize,
        120usize,
        frame,
        prev_frame,
    );
}

fn unpack_frame(prev_frame: &CptvFrame, frame: &mut CptvFrame, is_iframe: bool) {
    for y in 0..frame.image_data.height() {
        let is_odd = y % 2 == 0;
        if is_odd {
            for x in 0..frame.image_data.width() {
                frame.image_data[y][x] += predict_left(&frame.image_data, x, y) as u16;
            }
        } else {
            for x in (0..frame.image_data.width()).rev() {
                frame.image_data[y][x] += predict_right(&frame.image_data, x, y) as u16;
            }
        }
    }
    if !is_iframe {
        for y in 0..prev_frame.image_data.height() {
            for x in 0..prev_frame.image_data.width() {
                frame.image_data[y][x] += prev_frame.image_data[y][x];
            }
        }
    }
}
