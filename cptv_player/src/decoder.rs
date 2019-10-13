use cptv_common::{Cptv2Header, Cptv3, Cptv3Header, CptvFrame, FieldType, FrameData};
use log::{info, trace, warn};
use nom::bytes::complete::{tag, take};
use nom::number::complete::{le_f32, le_i16, le_u16, le_u32, le_u64, le_u8};
use FieldType::*;

pub fn decode_cptv3_header(i: &[u8]) -> nom::IResult<&[u8], Cptv3Header> {
    let mut meta = Cptv3Header::new();
    let (i, _) = tag(b"CPTV")(i)?;
    let (i, version) = le_u8(i)?;
    assert_eq!(version, 3);
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
            Timestamp => {
                meta.v2.timestamp = le_u64(val)?.1;
            }
            Width => {
                meta.v2.width = le_u32(val)?.1;
            }
            Height => {
                meta.v2.height = le_u32(val)?.1;
            }
            Compression => {
                meta.v2.compression = le_u8(val)?.1;
            }
            DeviceName => {
                meta.v2.device_name = String::from_utf8_lossy(val).into();
            }

            // Optional fields
            MotionConfig => {
                meta.v2.motion_config = Some(String::from_utf8_lossy(val).into());
            }
            PreviewSecs => {
                meta.v2.preview_secs = Some(le_u8(val)?.1);
            }
            Latitude => {
                meta.v2.latitude = Some(le_f32(val)?.1);
            }
            Longitude => {
                meta.v2.longitude = Some(le_f32(val)?.1);
            }
            LocTimestamp => {
                meta.v2.loc_timestamp = Some(le_u64(val)?.1);
            }
            Altitude => {
                meta.v2.altitude = Some(le_f32(i)?.1);
            }
            Accuracy => {
                meta.v2.accuracy = Some(le_f32(val)?.1);
            }
            // V3 fields
            MinValue => {
                meta.min_value = le_u16(val)?.1;
            }
            MaxValue => {
                meta.max_value = le_u16(val)?.1;
            }
            NumFrames => {
                meta.num_frames = le_u32(val)?.1;
            }
            FrameRate => {
                meta.frame_rate = le_u8(val)?.1;
            }
            FramesPerIframe => {
                meta.frames_per_iframe = le_u8(val)?.1;
            }
            x => panic!("Unknown header field type {:?}, {}", x, field),
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
    Ok((outer, meta))
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
        let field_code = FieldType::from(field_code);
        match field_code {
            TimeOn => {
                frame.time_on = le_u32(val)?.1;
            }
            PixelBytes => {
                frame.bit_width = le_u8(val)?.1;
            }
            FrameSize => {
                frame.frame_size = le_u32(val)?.1;
            }
            LastFfcTime => {
                frame.last_ffc_time = le_u32(val)?.1;
            }
            x => panic!("Unknown frame field type {:?} {:?}", x, frame),
        }
    }
    assert!(frame.frame_size > 0);
    let (i, data) = take(frame.frame_size as usize)(outer)?;
    copy_frame_data(data, &mut frame)?;
    unpack_frame(prev_frame, &mut frame, is_iframe);
    Ok((i, frame))
}

fn copy_frame_data<'a>(data: &'a [u8], frame: &mut CptvFrame) -> nom::IResult<&'a [u8], ()> {
    let mut data = data;
    if frame.bit_width == 1 {
        let mut i = 0;
        for y in 0..frame.image_data.height() {
            for x in 0..frame.image_data.width() {
                frame.image_data[y][x] = data[0] as i8 as i16;
            }
        }
    } else if frame.bit_width == 2 {
        for y in 0..frame.image_data.height() {
            for x in 0..frame.image_data.width() {
                let (remaining, val) = le_i16(data)?;
                data = remaining;
                frame.image_data[y][x] = val;
            }
        }
    }
    Ok((data, ()))
}

fn unpack_frame(prev_frame: &CptvFrame, frame: &mut CptvFrame, is_iframe: bool) {
    if is_iframe {
        // Decode snaking.
    }
    for y in 0..prev_frame.image_data.height() {
        for x in 0..prev_frame.image_data.width() {
            frame.image_data[y][x] += prev_frame.image_data[y][x];
        }
    }
}
