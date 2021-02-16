use libflate::gzip::Decoder;
use cptv_common::{Cptv2Header, FieldType, CptvFrame};
use crate::decoder::{CptvHeader, decode_frame_v2, get_dynamic_range};
use nom::bytes::complete::{tag, take};
use nom::number::complete::{le_f32, le_i8, le_u16, le_u32, le_u64, le_u8};
#[allow(unused)]
use log::{info, trace, warn};

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

fn get_raw_frame_v2(image_data: &mut [u8]) -> FrameHeaderV2 {
    let mut frame_header = FrameHeaderV2 {
        time_on: 0,
        last_ffc_time: 0,
        frame_number: 0,
        has_next_frame: false,
    };

    IFRAME_BLOCKS.with(|data| {
        // We only use the first block for V2
        let offset = PLAYBACK_INFO.with(|info| {
            let info = &*info.borrow();
            info.offset_in_block
        });
        let block = &data.borrow()[0];
        //info!("Get frame {}", number);
        // Read the frame out of the data:

        FRAME_BUFFER.with(|prev_frame| {
            let frame = {
                assert!(
                    offset <= block.len(),
                    "Offset is wrong {} vs {}",
                    offset,
                    block.len()
                );
                if let Ok((remaining, frame)) =
                decode_frame_v2(&prev_frame.borrow(), &block[offset..])
                {
                    image_data.copy_from_slice(frame.image_data.as_slice());
                    Some((remaining, frame))
                } else {
                    None
                }
            };
            if let Some((remaining, frame)) = frame {
                PLAYBACK_INFO.with(|info| {
                    let mut info = info.borrow_mut();
                    assert!(block.len() > remaining.len());
                    let offset = block.len() - remaining.len();
                    info.offset_in_block = usize::min(block.len(), offset);
                    info.prev_block = 0;
                    frame_header.last_ffc_time = frame.last_ffc_time;
                    frame_header.time_on = frame.time_on;
                    frame_header.frame_number = info.prev_frame as u32;
                    info.prev_frame += 1;
                });
                *prev_frame.borrow_mut() = frame;
            } else {
                *prev_frame.borrow_mut() = CptvFrame::new();
                PLAYBACK_INFO.with(|info| {
                    let mut info = info.borrow_mut();
                    // Restart playback at the beginning.
                    info.offset_in_block = 0;
                    info.prev_frame = 0;
                });
            }
        });
    });
    frame_header
}

fn get_frame_v2(number: u32, image_data: &mut [u8]) -> bool {
    let mut has_next_frame = false;
    IFRAME_BLOCKS.with(|data| {
        // We only use the first block for V2
        let offset = PLAYBACK_INFO.with(|info| {
            let info = &*info.borrow();
            info.offset_in_block
        });
        let block = &data.borrow()[0];
        //info!("Get frame {}", number);
        // Read the frame out of the data:

        FRAME_BUFFER.with(|prev_frame| {
            let frame = {
                assert!(
                    offset <= block.len(),
                    "Offset is wrong {} vs {}",
                    offset,
                    block.len()
                );
                if let Ok((remaining, frame)) =
                decode_frame_v2(&prev_frame.borrow(), &block[offset..])
                {
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
                PLAYBACK_INFO.with(|info| {
                    let mut info = info.borrow_mut();
                    assert!(block.len() > remaining.len());
                    let offset = block.len() - remaining.len();
                    info.offset_in_block = usize::min(block.len(), offset);
                    info.prev_block = 0;
                    let next_frame = number + 1;
                    info.prev_frame = next_frame as usize;
                });
                has_next_frame = true;
                *prev_frame.borrow_mut() = frame;
            } else {
                *prev_frame.borrow_mut() = CptvFrame::new();
                PLAYBACK_INFO.with(|info| {
                    let mut info = info.borrow_mut();
                    info.offset_in_block = 0;
                });
            }
        });
    });
    has_next_frame
}
