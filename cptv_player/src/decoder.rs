use cptv_common::{Cptv2, Cptv2Header, Cptv3, Cptv3Header, FieldType};
use log::{info, trace, warn};
use nom::bytes::complete::{tag, take};
use nom::number::complete::{le_f32, le_u16, le_u32, le_u64, le_u8};
use FieldType::*;

pub fn decode_cptv3_header(i: &[u8]) -> nom::IResult<&[u8], Cptv3Header> {
    let mut meta = Cptv3Header {
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
        min_value: std::u16::MAX,
        max_value: 0,
        toc: Vec::new(),
        num_frames: 0,
        frame_rate: 0,
        frames_per_iframe: 0,
    };

    let (i, _) = tag(b"CPTV")(i)?;
    let (i, version) = le_u8(i)?;
    assert_eq!(version, 3);
    let (i, _) = tag(b"H")(i)?;
    let (i, header_field_len_size) = le_u8(i)?;
    let (i, num_header_fields) = le_u8(i)?;
    use FieldType::*;
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
