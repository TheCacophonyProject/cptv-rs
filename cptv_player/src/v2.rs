use cptv_common::{Cptv2Header, FieldType, CptvFrame};
use crate::decoder::{CptvHeader, decode_frame_v2, get_dynamic_range};
use nom::bytes::streaming::{tag, take};
use wasm_bindgen::prelude::*;
use nom::number::streaming::{le_f32, le_u32, le_u64, le_u8};
#[allow(unused)]
use log::{info, trace, warn};
use crate::CptvPlayerContext;

pub fn decode_cptv2_header(i: &[u8]) -> nom::IResult<&[u8], CptvHeader> {
    let mut meta = Cptv2Header::new();
    let (i, val) = take(1usize)(i)?;
    let (_, _) = tag(b"H")(val)?;
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
                meta.device_id = Some(le_u32(val)?.1);
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
            FieldType::BackgroundFrame => {
                let has_background_frame = le_u8(val)?.1;
                // NOTE: We expect this to always be 1 if present
                meta.has_background_frame = Some(has_background_frame == 1);
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


#[wasm_bindgen]
pub struct FrameHeaderV2 {
    pub time_on: u32,
    pub last_ffc_time: u32,
    pub frame_number: u32,
    pub has_next_frame: bool,
}

pub fn get_raw_frame_v2(context: &mut CptvPlayerContext) -> FrameHeaderV2 {
    let mut frame_header = FrameHeaderV2 {
        time_on: 0,
        last_ffc_time: 0,
        frame_number: 0,
        has_next_frame: false,
    };
    // We only use the first block for V2
    //info!("Get frame {}", number);
    // Read the frame out of the data:
    let frame = {
        assert!(
            context.playback_info.offset_in_block <= context.iframe_blocks[0].len(),
            "Offset is wrong {} vs {}",
            context.playback_info.offset_in_block,
            context.iframe_blocks[0].len()
        );
        if let Ok((remaining, frame)) =
        decode_frame_v2(context.iframes.last(), &context.iframe_blocks[0][context.playback_info.offset_in_block..])
        {
            //image_data.copy_from_slice(frame.image_data.as_slice());
            Some((remaining, frame))
        } else {
            None
        }
    };

    if let Some((remaining, frame)) = frame {
        {
            assert!(context.iframe_blocks[0].len() > remaining.len());
            let offset = context.iframe_blocks[0].len() - remaining.len();
            context.playback_info.offset_in_block = usize::min(context.iframe_blocks[0].len(), offset);
            context.playback_info.prev_block = 0;
            frame_header.last_ffc_time = 1;//frame.last_ffc_time; // FIXME
            frame_header.time_on = frame.time_on;
            frame_header.frame_number = context.playback_info.prev_frame as u32;
            context.playback_info.prev_frame += 1;
        }
        context.frame_buffer = frame;
    } else {
        context.frame_buffer = CptvFrame::new();
        // Restart playback at the beginning.
        context.playback_info.offset_in_block = 0;
        context.playback_info.prev_frame = 0;
    }
    frame_header
}

pub fn get_frame_v2(context: &mut CptvPlayerContext, number: u32, image_data: &mut [u8]) -> bool {
    let mut has_next_frame = false;
    let frame = {
        assert!(
            context.playback_info.offset_in_block <= context.iframe_blocks[0].len(),
            "Offset is wrong {} vs {}",
            context.playback_info.offset_in_block,
            context.iframe_blocks[0].len()
        );
        let prev_frame = context.iframes.last();
        if let Ok((remaining, frame)) = decode_frame_v2(prev_frame, &context.iframe_blocks[0][context.playback_info.offset_in_block..]) {
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
        assert!(context.iframe_blocks[0].len() > remaining.len());
        let offset = context.iframe_blocks[0].len() - remaining.len();
        context.playback_info.offset_in_block = usize::min(context.iframe_blocks[0].len(), offset);
        context.playback_info.prev_block = 0;
        let next_frame = number + 1;
        context.playback_info.prev_frame = next_frame as usize;
        has_next_frame = true;
        context.frame_buffer = frame;
    } else {
        context.frame_buffer = CptvFrame::new();
        context.playback_info.offset_in_block = 0;
    }

    has_next_frame
}
