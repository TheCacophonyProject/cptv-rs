use cptv_common::{predict_left, predict_right, Cptv2Header, Cptv3Header, CptvFrame, FieldType, FrameData, WIDTH, HEIGHT};
#[allow(unused)]
use log::{info, trace, warn};
use nom::bytes::streaming::{tag, take};
use nom::number::streaming::{le_f32, le_i8, le_u16, le_u32, le_u64, le_u8};
use std::ops::RangeInclusive;

#[cfg(feature = "cptv2-support")]
use crate::v2::decode_cptv2_header;

#[cfg(feature = "cptv3-support")]
use crate::v3::decode_cptv3_header;
use std::num::NonZeroU32;

#[derive(Debug)]
pub enum CptvHeader {
    UNINITIALISED,
    V3(Cptv3Header),
    V2(Cptv2Header),
}

// TODO(jon): For completeness, we should be able to transcode a cptv2 file to cptv2 (with better compression?)

pub fn decode_cptv_header(i: &[u8]) -> nom::IResult<&[u8], CptvHeader> {
    let (i, val) = take(4usize)(i)?;
    let (_, _) = tag(b"CPTV")(val)?;
    let (i, version) = le_u8(i)?;
    match version {
        2 => decode_cptv2_header(i),
        3 => {
            {
                #[cfg(feature = "cptv3-support")]
                decode_cptv3_header(i)
            }
            {
                #[cfg(not(feature = "cptv3-support"))]
                panic!("cptv3 support not enabled")
            }
        },

        // TODO(jon): Should fail gracefully with a warning.
        _ => panic!("Unknown CPTV version {}", version),
    }
}

pub fn get_dynamic_range(frame: &FrameData) -> RangeInclusive<u16> {
    let mut frame_max = 0;
    let mut frame_min = std::u16::MAX;
    assert_eq!(frame.as_values().iter().count(), frame.width() * frame.height());
    for val in frame.as_values().iter()
    //.take(frame.width() * frame.height() - 36)
    // NOTE(jon): Offset
    {
        frame_max = u16::max(*val, frame_max);
        frame_min = u16::min(*val, frame_min);
    }
    frame_min..=frame_max
}

pub fn decode_frame_header_v2(data: &[u8], width: usize, height: usize) -> nom::IResult<&[u8], (&[u8], CptvFrame)> {
    let (i, val) = take(1usize)(data)?;
    let (_, _) = tag(b"F")(val)?;
    let (i, num_frame_fields) = le_u8(i)?;
    //info!("num frame fields {}", num_frame_fields);
    let mut frame = CptvFrame {
        time_on: 0,
        bit_width: 0,
        frame_size: 0,
        last_ffc_time: None,
        last_ffc_temp_c: None,
        frame_temp_c: None,
        is_background_frame: false,
        image_data: FrameData::with_dimensions(width, height),
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
                // NOTE: Last ffc time is relative to time_on, so we need to adjust it accordingly
                // when printing the value.
                frame.last_ffc_time = Some(le_u32(val)?.1);
            }
            FieldType::LastFfcTempC => {
                frame.last_ffc_temp_c = Some(le_f32(val)?.1);
            }
            FieldType::FrameTempC => {
                frame.frame_temp_c = Some(le_f32(val)?.1);
            }
            FieldType::BackgroundFrame => {
                frame.is_background_frame = le_u8(val)?.1 == 1;
            }
            _ => {
                warn!("Unknown frame field type '{}', length: {}", field_code as char, field_length);
            }
        }
    }
    assert!(frame.frame_size > 0);
    let (i, data) = take(frame.frame_size as usize)(outer)?;
    Ok((i, (data, frame)))
}

pub fn decode_frame_v2<'a>(
    prev_frame: Option<&CptvFrame>,
    data: &'a [u8],
) -> nom::IResult<&'a [u8], CptvFrame> {
    let (i, _) = tag(b"F")(data)?;
    let (i, num_frame_fields) = le_u8(i)?;
    let mut frame = CptvFrame {
        time_on: 0,
        bit_width: 0,
        frame_size: 0,
        last_ffc_time: None,
        last_ffc_temp_c: None,
        frame_temp_c: None,
        is_background_frame: false,
        image_data: FrameData::with_dimensions(160, 120),
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
                frame.last_ffc_time = Some(le_u32(val)?.1);
            }
            FieldType::LastFfcTempC => {
                frame.last_ffc_temp_c = Some(le_f32(val)?.1);
            }
            FieldType::FrameTempC => {
                frame.frame_temp_c = Some(le_f32(val)?.1);
            }
            _ => {
                warn!("Unknown frame field type '{}', length: {}", field_code as char, field_length);
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
        last_ffc_time: None,
        last_ffc_temp_c: None,
        frame_temp_c: None,
        is_background_frame: false,

        // TODO(jon): We need to initialise this to the correct dimensions, from our context?
        image_data: FrameData::with_dimensions(160, 120),
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
                frame.last_ffc_time = Some(le_u32(val)?.1);
            }
            FieldType::LastFfcTempC => {
                frame.last_ffc_temp_c = Some(le_f32(val)?.1);
            }
            FieldType::FrameTempC => {
                frame.frame_temp_c = Some(le_f32(val)?.1);
            }
            FieldType::BackgroundFrame => {
                let is_background_frame = le_u8(val)?.1;
                frame.is_background_frame = is_background_frame == 1;
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
    prev_frame: Option<&CptvFrame>,
) {
    match prev_frame {
        Some(prev_frame) => {
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
                let px = (prev_px + current_px) as u16;

                // This keeps track of min/max.
                frame.image_data.set(x, y, px);
                assert!(y * width + x <= width * height);
            }
        }
        None => {
            // This is the first frame, so we don't need to use a previous frame
            frame.image_data[0][0] = current_px as u16;
            for (index, delta) in BitUnpacker::new(i, frame.bit_width)
                .take((width * height) - 1)
                .enumerate()
            {
                let index = index + 1;
                let y = index / width;
                let x = index % width;
                let x = if y & 1 == 1 { width - x - 1 } else { x };
                current_px += delta;
                let px = current_px as u16;

                // This keeps track of min/max.
                frame.image_data.set(x, y, px);
                assert!(y * width + x <= width * height);
            }
        }
    }
}

pub fn unpack_frame_v2(prev_frame: Option<&CptvFrame>, data: &[u8], frame: &mut CptvFrame) {
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
        frame.image_data.width(),
        frame.image_data.height(),
        frame,
        prev_frame,
    );
}

// Unpack a frame based on the previous frame:
// TODO(jon): Shouldn't this be predicated on whether this is a v2 or v3 frame?
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
