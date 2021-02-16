use ruzstd::frame_decoder;
use nom::bytes::complete::{tag, take};
use nom::number::complete::{le_f32, le_i8, le_u16, le_u32, le_u64, le_u8};
use cptv_common::{Cptv3Header, FieldType};
use std::io::Cursor;
use crate::decoder::{CptvHeader, decode_frame};

fn decode_zstd_blocks(meta: &Cptv3Header, remaining: &[u8]) -> Vec<Vec<u8>> {
    let mut iframe_blocks = Vec::new();
    let mut prev_offset = 0;

    for pos in &meta.toc[1..] {
        let pos = *pos as usize;
        iframe_blocks.push(&remaining[prev_offset..pos]);
        prev_offset = pos;
    }
    iframe_blocks.push(&remaining[prev_offset..]);

    // Should we just decode frame blocks on demand, or up front?
    // Now decode and play frames!

    // Event loop here to request decoding and drawing of frames.
    // What is the best way to listen for requests from the UI?
    // Probably a request animation frame loop, right?
    let mut decoded_zstd_blocks = Vec::new();
    for iframe_block in iframe_blocks {
        let mut frame_dec = frame_decoder::FrameDecoder::new();
        let mut f = Cursor::new(iframe_block);
        frame_dec.init(&mut f).unwrap();
        frame_dec
            .decode_blocks(&mut f, frame_decoder::BlockDecodingStrategy::All)
            .unwrap();
        if let Some(result) = frame_dec.collect() {
            decoded_zstd_blocks.push(result);
        }
    }
    decoded_zstd_blocks
}


fn get_frame_v3(number: u32, image_data: &mut [u8]) -> bool {
    // Find the block closest, decode from the start to frame x:
    let (mut offset, prev_block, prev_frame_num) = PLAYBACK_INFO.with(|info| {
        let info = info.borrow();
        (info.offset_in_block, info.prev_block, info.prev_frame)
    });
    let prev_frame_num = if number as usize != prev_frame_num {
        // We got a seek.
        number as usize
    } else {
        prev_frame_num
    };
    // let (max, min, frames_per_iframe, num_frames, num_blocks) = CLIP_INFO.with(|meta| {
    //     let meta = meta.borrow();
    //     (
    //         meta.max_value,
    //         meta.min_value,
    //         meta.frames_per_iframe,
    //         meta.num_frames,
    //         meta.toc.len(),
    //     )
    // });
    let (max, min, frames_per_iframe, num_frames) = (
        get_max_value(),
        get_min_value(),
        get_frames_per_iframe(),
        get_num_frames(),
    );
    let block_num = (prev_frame_num as u32 / frames_per_iframe as u32) as usize;
    if block_num != prev_block {
        offset = 0;
    }
    let inv_dynamic_range = 1.0 / (max - min) as f32;
    IFRAME_BLOCKS.with(|data| {
        let block = &data.borrow()[block_num];
        // Read the frame out of the data:
        FRAME_BUFFER.with(|prev_frame| {
            let frame = {
                if let Ok((remaining, frame)) =
                decode_frame(&prev_frame.borrow(), &block[offset..], offset == 0)
                {
                    let image = &frame.image_data;
                    // Copy frame out to output:
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
                    info.offset_in_block = block.len() - remaining.len();
                    info.prev_block = block_num;
                    let next_frame = usize::min(num_frames as usize, prev_frame_num + 1);
                    info.prev_frame = next_frame;
                });
                *prev_frame.borrow_mut() = frame;
            }
        });
    });
    true
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
                warn!("Unknown header field type {}", field)
                //std::process::abort();
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
