use byteorder::WriteBytesExt;
use cptv_common::{
    predict_left, predict_right, Cptv2, Cptv2Header, CptvFrame, FieldType, FrameData,
};

use discrete_transforms::dct_2d::Dct2D;
use libflate::gzip::Encoder;
use libflate::gzip::{Decoder, EncodeOptions};
use libflate::lz77::DefaultLz77Encoder;
use libflate::zlib::Lz77WindowSize;
#[allow(unused)]
use log::{info, trace, warn};
use nom::number::complete::{le_f32, le_u32, le_u64, le_u8};
use nom::{bytes::complete::*, Err};
use std::collections::HashMap;
use std::fmt::{Error, Formatter};
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::ops::{Index, IndexMut, Range, RangeInclusive};
use std::path::Path;
use std::{fmt, fs};

fn main() -> std::result::Result<(), std::boxed::Box<dyn std::error::Error>> {
    env_logger::init();
    // 20190922-021028
    // 20190922-021916
    //let input_name = "20191016-223709";
    // let input_name = "20191020-062935";
    //let input_name = "20200317-222551";
    //let input_name = "20200525.134708.107";
    let input_name = "20201118-060646";
    let input_name = "wallabys-20201008-000650";
    let input_name = "wallabys-20201008-004117";
    let input_name = "bunny_20201125-060544";
    let input_name = "reasonably-static-20201216-060355";
    //let input_name = "20190922-021028";
    match fs::read(format!("{}.cptv", input_name)) {
        Ok(input) => {
            info!("Input size {}", input.len());
            let mut gz_decoder = Decoder::new(&input[..])?;
            let mut decoded = Vec::new();
            gz_decoder.read_to_end(&mut decoded)?;
            let raw_len = decoded.len();
            match decode_cptv(&decoded) {
                Ok((_, cptv)) => {
                    // TODO(jon): Fix offset artifact issues.
                    //dump_png_frames(&cptv);
                    try_compression(&cptv, &input_name, input.len(), raw_len);
                }
                Err(Err::Error((remaining, e))) => {
                    println!("err {:?}, remaining {}", e, remaining.len())
                }
                Err(Err::Incomplete(needed)) => println!("incomplete {:?}", needed),
                Err(Err::Failure((_, e))) => println!("failure {:?}", e),
            }
            Ok(())
        }
        Err(message) => {
            println!("{}", message);
            Ok(())
        }
    }
}

// TODO(jon): Why isn't this using the decoder::decode_cptv2_header function?
fn decode_header(i: &[u8]) -> nom::IResult<&[u8], Cptv2Header> {
    let mut meta = Cptv2Header {
        timestamp: 0,
        width: 0,
        height: 0,
        compression: 0,
        device_name: String::new(),
        device_id: None,
        fps: Some(9), // Default
        serial_number: None,
        brand: None,
        model: None,
        motion_config: None,
        preview_secs: None,
        latitude: None,
        longitude: None,
        loc_timestamp: None,
        altitude: None,
        accuracy: None,
        firmware_version: None,
    };

    let (i, _) = tag(b"CPTV")(i)?;
    let (i, version) = le_u8(i)?;
    assert_eq!(version, 2);
    let (i, _) = tag(b"H")(i)?;
    let (i, num_header_fields) = le_u8(i)?;

    //dbg!(num_header_fields);
    let mut outer = i;
    for _ in 0..num_header_fields as usize {
        let (i, field_length) = le_u8(outer)?;
        let (i, field) = le_u8(i)?;
        let (i, val) = take(field_length)(i)?;
        outer = i;
        // dbg!(field_length);
        // dbg!(format!("{:?}", field as char));
        match field {
            b'T' => {
                meta.timestamp = le_u64(val)?.1;
            }
            b'X' => {
                meta.width = le_u32(val)?.1;
            }
            b'Y' => {
                meta.height = le_u32(val)?.1;
            }
            b'C' => {
                meta.compression = le_u8(val)?.1;
            }
            b'D' => {
                meta.device_name = String::from_utf8_lossy(val).into();
            }

            // Optional fields
            b'I' => {
                meta.device_id = Some(String::from_utf8_lossy(val).into());
            }
            b'M' => {
                meta.motion_config = Some(String::from_utf8_lossy(val).into());
            }
            b'P' => {
                meta.preview_secs = Some(le_u8(val)?.1);
            }
            b'L' => {
                meta.latitude = Some(le_f32(val)?.1);
            }
            b'O' => {
                meta.longitude = Some(le_f32(val)?.1);
            }
            b'S' => {
                meta.loc_timestamp = Some(le_u64(val)?.1);
            }
            b'A' => {
                meta.altitude = Some(le_f32(i)?.1);
            }
            b'U' => {
                meta.accuracy = Some(le_f32(val)?.1);
            }
            b'E' => {
                meta.model = Some(String::from_utf8_lossy(val).into());
            }
            b'B' => {
                meta.brand = Some(String::from_utf8_lossy(val).into());
            }
            b'Z' => {
                meta.fps = Some(le_u8(val)?.1);
            }
            // b'N' => {
            //     // TODO(jon)
            // }
            // b'V' => {
            //     // TODO(jon)
            // }
            x => warn!("Unknown header field type {}", x as char),
        }
    }
    Ok((outer, meta))
}

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
    prev_frame: &Option<&CptvFrame>,
) {
    // Take the first 4 bytes as initial delta value
    let prev_px = if let Some(prev_frame) = prev_frame {
        prev_frame.image_data[0][0]
    } else {
        0
    };
    // Seed the initial pixel value
    assert!(prev_px as i32 + current_px <= std::i16::MAX as i32);
    frame.image_data[0][0] = (prev_px as i32 + current_px) as u16;
    for (index, delta) in BitUnpacker::new(i, frame.bit_width)
        .take((width * height) - 1)
        .enumerate()
    {
        let index = index + 1;
        let y = index / width;
        let x = index % width;
        let x = if y & 1 == 1 { width - x - 1 } else { x };
        current_px += delta;
        let prev_px = if let Some(prev_frame) = prev_frame {
            prev_frame.image_data[y][x]
        } else {
            0
        };
        assert!(prev_px as i32 + current_px <= std::i16::MAX as i32);
        frame.image_data[y][x] = (prev_px as i32 + current_px) as u16;
    }
}

fn decode_frame<'a>(
    i: &'a [u8],
    width: u32,
    height: u32,
    prev_frame: &Option<&CptvFrame>,
) -> nom::IResult<&'a [u8], CptvFrame> {
    let (i, _) = tag(b"F")(i)?;
    let (i, num_frame_fields) = le_u8(i)?;
    let mut frame = CptvFrame {
        time_on: 0,
        bit_width: 0,
        frame_size: 0,
        last_ffc_time: 0,
        image_data: FrameData::with_dimensions(),
    };
    let mut outer = i;
    for _ in 0..num_frame_fields as usize {
        let (i, field_length) = le_u8(outer)?;
        let (i, field_code) = le_u8(i)?;
        let (i, val) = take(field_length)(i)?;
        outer = i;
        match field_code {
            b't' => {
                frame.time_on = le_u32(val)?.1;
            }
            b'w' => {
                frame.bit_width = le_u8(val)?.1;
            }
            b'f' => {
                frame.frame_size = le_u32(val)?.1;
            }
            b'c' => {
                frame.last_ffc_time = le_u32(val)?.1;
            }
            b'a' => {
                // TODO(jon)
            }
            b'b' => {
                // TODO(jon)
            }
            x => panic!(
                "Unknown frame field type '{}'",
                String::from_utf8_lossy(&[x])
            ),
        }
    }
    assert!(frame.frame_size > 0);
    let (i, data) = take(frame.frame_size as usize)(outer)?;

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
        width as usize,
        height as usize,
        &mut frame,
        prev_frame,
    );
    Ok((i, frame))
}

fn decode_frames(i: &[u8], width: u32, height: u32) -> nom::IResult<&[u8], Vec<CptvFrame>> {
    let mut frames = Vec::new();
    let mut prev_frame: Option<&CptvFrame> = None;
    let mut i = i;
    while i.len() != 0 {
        let (remaining, frame) = decode_frame(i, width, height, &prev_frame)?;
        i = remaining;
        frames.push(frame);
        prev_frame = frames.last();
    }

    // for frame in &mut frames {
    //     frame.image_data = frame.image_data.offset(36);
    // }

    Ok((i, frames))
}

fn decode_cptv(i: &[u8]) -> nom::IResult<&[u8], Cptv2> {
    // For reading and opening files
    let (i, meta) = decode_header(i)?;
    let (i, frames) = decode_frames(i, meta.width, meta.height)?;
    assert_eq!(i.len(), 0);
    Ok((i, Cptv2 { frames, meta }))
}

fn get_dynamic_range(frame: &FrameData) -> RangeInclusive<i16> {
    let mut frame_max = 0;
    let mut frame_min = std::i16::MAX;

    for val in frame
        .as_values()
        .iter()
        .take(frame.width() * frame.height() - 36)
    // NOTE(jon): Offset
    {
        assert!((*val as i16) < std::i16::MAX);
        frame_max = i16::max(*val as i16, frame_max);
        frame_min = i16::min(*val as i16, frame_min);
    }
    frame_min..=frame_max
}

fn dump_png_frames(cptv: &Cptv2) {
    // Work out the dynamic range to scale here:
    let mut min = std::i16::MAX;
    let mut max = 0;
    for frame in &cptv.frames {
        let frame_range = get_dynamic_range(&frame.image_data);
        min = i16::min(*frame_range.start(), min);
        max = i16::max(*frame_range.end(), max);
    }
    info!(
        "dynamic range across entire clip {:?}, range {}",
        min..=max,
        (min..max).len()
    );

    let inv_dynamic_range = 1.0 / (max - min) as f32;
    for (index, frame) in cptv.frames.iter().enumerate() {
        let p = format!("out/image_{}.png", index);
        let path = Path::new(&p);
        let file = File::create(path).unwrap();
        let w = BufWriter::new(file);
        let mut encoder = png::Encoder::new(w, cptv.meta.width, cptv.meta.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let mut data = Vec::new();
        for y in 0..cptv.meta.height as usize {
            for x in 0..cptv.meta.width as usize {
                let val =
                    ((frame.image_data[y][x] as f32 - min as f32) * inv_dynamic_range) * 255.0;
                data.push(val as u8);
            }
        }
        writer.write_image_data(&data).unwrap();
    }
}

fn pack_frame(frames: &mut Vec<Vec<u8>>, frame: FrameData, meta: &CptvFrame, is_iframe: bool) {
    // Work out whether this frame can be easily represented in i8 space, using one byte per pixel.
    let frame_range = get_dynamic_range(&frame);
    let pixel_bytes = if frame_range.len() <= std::u8::MAX as usize
        && *frame_range.start() >= std::i8::MIN as i16
        && *frame_range.end() <= std::i8::MAX as i16
    {
        1u8
    } else {
        2u8
    };

    // NOTE(jon): We only want to do this if the values in the frame can all be represented as
    //  i8s without any offsetting: offsetting other values that do have a dynamic range <= 255
    //  would still skew our data away from having most of the delta values be around 0, and actually
    //  hurts compressibility, since it varies the data more between frames.
    let mut bytes = Vec::new();

    // Write the frame header
    push_field(&mut bytes, &4u8, FieldType::FrameHeader);
    push_field(
        &mut bytes,
        &(frame.width() * frame.height() * pixel_bytes as u32),
        FieldType::FrameSize,
    );
    // NOTE(jon): Frame size is technically redundant, as it will always be width * height * pixel_bytes
    push_field(&mut bytes, &pixel_bytes, FieldType::PixelBytes);
    push_field(&mut bytes, &meta.time_on, FieldType::TimeOn);
    push_field(&mut bytes, &meta.last_ffc_time, FieldType::LastFfcTime);
    if pixel_bytes == 1 {
        // Seems fair to say that most frames fit comfortably inside 8 bits.
        for y in 0..frame.height() {
            for x in 0..frame.width() {
                let val = frame[y][x] as i8 as u8;
                bytes.push(val);
            }
        }
    } else {
        bytes.extend_from_slice(frame.as_slice());
    }
    frames.push(bytes);
}

fn push_field<T: Sized>(output: &mut Vec<u8>, value: &T, code: FieldType) -> usize {
    let size = std::mem::size_of_val(value);
    //println!("adding field {:?} at {}", code, output.len());
    output.push(code as u8);
    output.push(size as u8);
    let value_offset = output.len();
    output.extend_from_slice(unsafe {
        std::slice::from_raw_parts(value as *const T as *const u8, size)
    });
    value_offset
}

fn push_toc(output: &mut Vec<u8>, value: &[u32], code: FieldType) {
    use byteorder::LittleEndian;
    assert!(code == FieldType::TableOfContents);
    output.push(code as u8);
    output
        .write_u32::<LittleEndian>(value.len() as u32)
        .unwrap();
    for v in value {
        output.write_u32::<LittleEndian>(*v).unwrap();
    }
}

fn push_string(output: &mut Vec<u8>, value: &str, code: FieldType) {
    output.push(code as u8);
    output.push(value.len() as u8);
    output.extend_from_slice(value.as_bytes());
}

fn predictor_average_2(a: i16, b: i16) -> i16 {
    (a + b) / 2
}

fn try_compression(cptv: &Cptv2, file_name: &str, input_len: usize, raw_len: usize) {
    let mut frames_size = 0;
    let seconds_between_iframes = 5;
    let i_frame_interval = 9 * seconds_between_iframes;
    let mut delta_frames = Vec::new();
    let delta_fn = delta_compress_identity;
    //let delta_fn = delta_compress_frame_snaking; //delta_compress_lines_with_prediction;
    //let iframe_fn = delta_compress_lines_with_prediction;
    let mut num_iframes = 0;

    // Dynamic range:
    let mut min = std::i16::MAX;
    let mut max = 0;

    for frame in &cptv.frames {
        let frame_range = get_dynamic_range(&frame.image_data);
        min = i16::min(*frame_range.start(), min);
        max = i16::max(*frame_range.end(), max);
    }

    // IDEA: The first "frame" of each block should be a composite image representing the median pixel values of the block.
    // Each set of subsequent frames would have the values as they deviate from this median.  Would average be better?

    // IDEA: For a block of pixels, would it help to shuffle the MSBs to all be next to each other,
    // Then have the LSBs next to each other?  Then delta encode both of these separately?
    // This could probably be sped up with various shuffle instructions?

    // IDEA: Should we make the iframe chunks a fixed number of frames, or aim for a target average size instead?

    // TRY: Adaptive delta coding with escape codes.
    // Also, if we know the frame min, we could subtract that first from all values, so we're dealing with lower values?

    // TRY: BWT + MTF transformations.

    // IDEA: See if we can model some prediction for pixel noise/change per frame, and then encode the difference from
    // our prediction to reality?

    let num_frames = cptv.frames.len();
    info!("Num frames {}", num_frames);
    let mut p = vec![0usize; 155];

    //let mut prev_frame = FrameData::empty();
    for (frame_index, frames) in cptv.frames.windows(2).enumerate() {
        let is_first_frame = frame_index == 0;

        let is_i_frame = frame_index % i_frame_interval == 0;

        let frame_a = &frames[0];
        let frame_b = &frames[1];

        p[frame_index] += frame_a.image_data[60][60] as usize;

        if is_first_frame {
            // DO DCT:
            // grab 8x8 blocks
            let mut first = true;
            let d_f = delta_fn(&frame_a.image_data);
            //info!("Input 0 {:?}", &d_f.as_values()[200..208]);
            // for block in d_f.blocks() {
            //     //info!("Got block");
            //     use rustdct::DCTplanner;
            //     use std::sync::Arc;
            //     let mut output = vec![0.0f32; 64];
            //     let mut input: Vec<f32> = block.iter().map(|x| *x as f32).collect();
            //     if first {
            //         info!("input pre {:?}", input);
            //     }
            //
            //     let mut planner = DCTplanner::new();
            //     let mut dct = planner.plan_dct2(64);
            //     dct.process_dct2(&mut input, &mut output);
            //     if first {
            //         first = false;
            //         info!("input {:?}", input);
            //         info!("output {:?}", output);
            //     }
            // }
        }

        // Delta between frames, then in frame?
        if is_first_frame {
            pack_frame(
                &mut delta_frames,
                delta_fn(&frame_a.image_data),
                &frame_a,
                true,
            );
            num_iframes += 1;
        } else if is_i_frame {
            pack_frame(
                &mut delta_frames,
                delta_fn(&frame_b.image_data),
                &frame_b,
                true,
            );
            num_iframes += 1;
        } else {
            let mut d: FrameData = FrameData::with_dimensions();
            for y in 0..cptv.meta.height as usize {
                for x in 0..cptv.meta.width as usize {
                    d[y][x] = frame_b.image_data[y][x] - frame_a.image_data[y][x];
                }
            }
            pack_frame(&mut delta_frames, delta_fn(&d), &frame_b, false);
        }
    }

    let mut mm = p.clone();
    mm.sort();
    let m = mm[p.len() / 2];

    info!("Values {:#?}", p[p.len() / 2]);
    info!(
        "offset from median {:#?}, energy {}, {:?}",
        p.iter()
            .map(|x| m as isize - *x as isize)
            .collect::<Vec<_>>(),
        p.iter().map(|x| m as isize - *x as isize).sum::<isize>(),
        &p.windows(2)
            .map(|a| (a[1] as isize - m as isize) - (a[0] as isize - m as isize))
            .collect::<Vec<_>>(),
    );

    let average: isize = (p.iter().sum::<usize>() / p.len()) as isize;

    info!(
        "offset from average {:#?}, energy {}, {:?}",
        p.iter().map(|x| *x as isize - average).collect::<Vec<_>>(),
        p.iter().map(|x| *x as isize - average).sum::<isize>(),
        &p.windows(2)
            .map(|a| (a[1] as isize - average) - (a[0] as isize - average))
            .collect::<Vec<_>>(),
    );
    info!(
        "{:?}",
        &p.windows(2)
            .map(|a| (a[1] as isize) - (a[0] as isize))
            .collect::<Vec<_>>()
    );

    info!("0b{:b}, 0b{:b}, 0b{:b}", 1i8, 0i8, -1i8);

    // IDEA(jon): Intra-frame per-pixel RLE for each block?

    // IDEA: What if we organise all the pixels in a block as a linear stream?  That would mean we
    // can't start decoding a block until the whole thing has downloaded, but that is probably okay
    // if blocks are short (9 frames, 1 second?)

    // NOTE(jon): Since we are only making it so you can go to the beginning of each iframe to start
    //  decode, we should also make the subsequent frames up until the next iframe part of the zstd
    //  compression, for additional size reductions.
    let mut compressed_data = Vec::new();
    let num_frames = delta_frames.len();
    let mut first_in_range = 0;
    let mut intermediate_frame_buffer = Vec::new();
    let mut iframe_offsets = Vec::new();
    for (frame_index, frame) in delta_frames.iter().enumerate() {
        let is_i_frame = frame_index % i_frame_interval == 0;

        let is_first_frame = frame_index == 0;
        let is_last_frame = frame_index == num_frames - 1;
        if (is_last_frame || is_i_frame) && !is_first_frame {
            let compressed = zstd::encode_all(&intermediate_frame_buffer[..], 16);
            if let Ok(compressed) = compressed {
                frames_size += compressed.len();
                println!(
                    "Zstd frame range {:?} frames, {:?}: {} bytes",
                    (first_in_range..frame_index).len(),
                    first_in_range..frame_index,
                    compressed.len()
                );
                iframe_offsets.push(compressed_data.len() as u32);
                compressed_data.extend_from_slice(&compressed);
            }
            intermediate_frame_buffer.clear();
            first_in_range = frame_index;
        }
        intermediate_frame_buffer.extend_from_slice(frame);
    }

    let mut output: Vec<u8> = Vec::new();
    // TODO(jon): Write an uncompressed TOC here, with the offsets of all iframes in the compressed
    //  stream.

    output.extend_from_slice(&b"CPTV"[..]);
    output.push(3);
    let mut num_fields = 0;
    let num_fields_offset = push_field(&mut output, &num_fields, FieldType::Header);
    push_field(&mut output, &cptv.meta.timestamp, FieldType::Timestamp);
    push_field(&mut output, &cptv.meta.width, FieldType::Width);
    push_field(&mut output, &cptv.meta.height, FieldType::Height);
    push_field(&mut output, &cptv.meta.compression, FieldType::Compression);
    push_field(&mut output, &min, FieldType::MinValue);
    push_field(&mut output, &max, FieldType::MaxValue);
    let frames_per_iframe = i_frame_interval as u8;
    push_field(&mut output, &frames_per_iframe, FieldType::FramesPerIframe);
    push_field(&mut output, &9u8, FieldType::FrameRate);
    let num_frames = cptv.frames.len() as u32;
    push_field(&mut output, &num_frames, FieldType::NumFrames);

    push_string(&mut output, &cptv.meta.device_name, FieldType::DeviceName);
    num_fields += 10;

    if let Some(motion_config) = &cptv.meta.motion_config {
        push_string(&mut output, motion_config, FieldType::MotionConfig);
        num_fields += 1;
    }
    if let Some(preview_secs) = &cptv.meta.preview_secs {
        push_field(&mut output, preview_secs, FieldType::PreviewSecs);
        num_fields += 1;
    }
    if let Some(latitude) = &cptv.meta.latitude {
        push_field(&mut output, latitude, FieldType::Latitude);
        num_fields += 1;
    }
    if let Some(longitude) = &cptv.meta.longitude {
        push_field(&mut output, longitude, FieldType::Longitude);
        num_fields += 1;
    }
    if let Some(loc_timestamp) = &cptv.meta.loc_timestamp {
        push_field(&mut output, loc_timestamp, FieldType::LocTimestamp);
        num_fields += 1;
    }
    if let Some(altitude) = &cptv.meta.altitude {
        push_field(&mut output, altitude, FieldType::Altitude);
        num_fields += 1;
    }
    if let Some(accuracy) = &cptv.meta.accuracy {
        push_field(&mut output, accuracy, FieldType::Accuracy);
        num_fields += 1;
    }
    println!("Output {} fields", num_fields);
    output[num_fields_offset] = num_fields;

    // Length will be num_iframes * sizeof u32, and will have offsets into the compressed stream of
    // where each iframe begins, from the start of the file, or maybe from the end of the TOC, which
    // should be the last section.  This means we can rewrite header metadata without rewriting TOC.
    println!("Iframe offsets, {:?}", iframe_offsets);
    push_toc(&mut output, &iframe_offsets, FieldType::TableOfContents);
    output.extend_from_slice(&compressed_data);
    println!("All frames Zstd: {}", output.len());
    println!(
        "Ratio V2 -> V3 1:{}",
        input_len as f32 / output.len() as f32
    );
    println!("Ratio Raw -> V2 1:{}", raw_len as f32 / input_len as f32);
    println!("Ratio Raw -> V3 1:{}", raw_len as f32 / output.len() as f32);
    let mut file = File::create(format!("{}-v3.cptv", file_name)).unwrap();
    file.write_all(&output).unwrap();
    /*
    {
        // Basic gzip for comparison, probably not using best compression algorithm.
        let mut frames_size = 0;
        let num_frames = delta_frames.len();
        let mut first_in_range = 0;
        for (frame_index, frame) in delta_frames.iter().enumerate() {
            let is_i_frame = frame_index % i_frame_interval == 0;
            let is_first_frame = frame_index == 0;
            let is_last_frame = frame_index == num_frames - 1;
            if (is_last_frame || is_i_frame) && !is_first_frame {
                let mut encoder = Encoder::new(Vec::new()).unwrap();
                encoder.write_all(&output[..]).unwrap();
                let compressed = encoder.finish().into_result().unwrap();
                frames_size += compressed.len();
                println!(
                    "Zlib frame range {:?} frames, {:?}: {} bytes",
                    (first_in_range..frame_index).len(),
                    first_in_range..frame_index,
                    compressed.len()
                );
                first_in_range = frame_index;
            }
        }
        println!("All frames Zlib {}", frames_size);
    }
    */

    // NOTE(jon): Compressing all frame data in a contiguous block is actually larger than splitting
    //  it at iframes.
    /*
    let mut frames_size = 0;
    for (frame_index, frame) in delta_frames.iter().enumerate() {
        let compressed = zstd::encode_all(frame.as_slice(), 14);
        if let Ok(compressed) = compressed {
            frames_size += compressed.len();
            //println!("Zstd frame #{}: {}", frame_index, compressed.len());
        }
    }
    println!("All frames individually compressed {}", frames_size);
    */
}

fn delta_compress_identity(data: &FrameData) -> FrameData {
    data.clone()
}

// TODO(jon): Delta compress blocks of 4*4?
//  - Maybe exploit the fact that we have smaller regions of variance in blocks as opposed to lines?
//  - Sum the output of delta encoding between frames vs in-frame.
//  - It *might* be worth reducing the number of input bytes if variance fits in 8 bits as opposed to 16.

fn delta_compress_lines(data: &FrameData) -> FrameData {
    let mut enc = FrameData::with_dimensions();
    for y in 0..data.height() {
        let mut prev = 0;
        for x in 0..data.width() {
            enc[y][x] = data[y][x] - prev;
            prev = data[y][x];
        }
    }
    // Verify delta encoding:
    let mut dec = FrameData::with_dimensions();
    for y in 0..data.height() {
        let mut prev = 0;
        for x in 0..data.width() {
            dec[y][x] = enc[y][x] + prev;
            prev = dec[y][x];
        }
    }
    assert_eq!(data.as_slice(), dec.as_slice());
    enc
}

fn predict_9x9(data: &FrameData, x: usize, y: usize) -> i16 {
    let width = data.width();
    let height = data.height();
    let left = if x == 0 { 0 } else { data[y][x - 1] };
    let top = if y == 0 { 0 } else { data[y - 1][x] };
    let top_left = if y == 0 || x == 0 {
        0
    } else {
        data[y - 1][x - 1]
    };
    let top_right = if x == width - 1 || y == 0 {
        0
    } else {
        data[y - 1][x + 1]
    };
    let middle = data[y][x];
    let right = if x == width - 1 { 0 } else { data[y][x + 1] };
    let bottom_right = if x == width - 1 || y == height - 1  {
        0
    } else {
        data[y + 1][x + 1]
    };
    let bottom = if y == height - 1 { 0 } else { data[y + 1][x] };
    let bottom_left = if y == height - 1 || x == 0 {
        0
    } else {
        data[y + 1][x - 1]
    };

    let p1 = predictor_average_2(left as i16, top_left as i16);
    let p2 = predictor_average_2(top as i16, top_right as i16);
    let p3 = predictor_average_2(right as i16, bottom_right as i16);
    let p4 = predictor_average_2(bottom as i16, bottom_left as i16);

    let p5 = predictor_average_2(p1, p2);
    let p6 = predictor_average_2(p3, p4);
    let p7 = predictor_average_2(p5, p6);
    predictor_average_2(p7, middle as i16)
}

fn predict_average(data: &FrameData, x: usize, y: usize) -> i16 {
    let left = if x == 0 { 0 } else { data[y][x - 1] as isize };
    let top = if y == 0 { 0 } else { data[y - 1][x] as isize };
    let width = data.width();
    let top_left = if y == 0 || x == 0 {
        0
    } else {
        data[y - 1][x - 1] as isize
    };
    let top_right = if x == width - 1 || y == 0 {
        0
    } else {
        data[y - 1][x + 1] as isize
    };
    ((left + top + top_left + top_right) / 4) as i16
}

fn delta_compress_lines_with_prediction(data: &FrameData) -> FrameData {
    let mut enc = FrameData::with_dimensions();
    for y in 0..data.height() {
        for x in 0..data.width() {
            enc[y][x] = ((data[y][x] as i16) - predict_left(&data, x, y)) as u16;
        }
    }
    // Verify delta encoding:
    let mut dec = FrameData::with_dimensions();
    for y in 0..data.height() {
        for x in 0..data.width() {
            dec[y][x] = ((enc[y][x] as i16) + predict_left(&dec, x, y)) as u16;
        }
    }
    assert_eq!(data.as_slice(), dec.as_slice());
    enc
}

fn delta_compress_frame_snaking(data: &FrameData) -> FrameData {
    let mut enc = FrameData::with_dimensions();
    for y in 0..data.height() {
        let is_odd = y % 2 == 0;
        if is_odd {
            for x in 0..data.width() {
                enc[y][x] = ((data[y][x] as i16) - predict_left(&data, x, y)) as u16;
            }
        } else {
            for x in (0..data.width()).rev() {
                enc[y][x] = ((data[y][x] as i16) - predict_right(&data, x, y)) as u16;
            }
        }
    }

    // Verify delta encoding:
    let mut dec = FrameData::with_dimensions();
    for y in 0..data.height() {
        let is_odd = y % 2 == 0;
        if is_odd {
            for x in 0..data.width() {
                dec[y][x] = ((enc[y][x] as i16) + predict_left(&dec, x, y)) as u16;
            }
        } else {
            for x in (0..data.width()).rev() {
                dec[y][x] = ((enc[y][x] as i16) + predict_right(&dec, x, y)) as u16;
            }
        }
    }
    assert_eq!(data.as_slice(), dec.as_slice());
    enc
}

// TODO(jon): Decode again, and confirm that we're not missing anything.
