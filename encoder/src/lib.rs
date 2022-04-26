use chrono::DateTime;
use cptv_shared::v2::types::{Cptv2Header, CptvFrame, FieldType, FrameData};
use js_sys::{Reflect, Uint8Array};
use log::info;
use log::Level;
use std::io::Write;
use std::mem;
use flate2::write::GzEncoder;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use cptv_shared::CptvHeader;
use cptv_shared::CptvHeader::{V2, V3};

const X: u16 = 64u16;

const O: u16 = 1u16;

const ZERO: [u16; 15] = [
    O, X, X,
    X, O, X,
    X, O, X,
    X, O, X,
    X, X, O,
];

const ONE: [u16; 15] = [
    O, X, O,
    X, X, O,
    O, X, O,
    O, X, O,
    O, X, O,
];

const TWO: [u16; 15] = [
    O, X, O,
    X, O, X,
    O, O, X,
    X, X, O,
    X, X, X,
];

const THREE: [u16; 15] = [
    X, X, O,
    O, O, X,
    X, X, O,
    O, O, X,
    X, X, O,
];

const FOUR: [u16; 15] = [
    X, O, X,
    X, O, X,
    X, X, X,
    O, O, X,
    O, O, X,
];

const FIVE: [u16; 15] = [
    X, X, X,
    X, O, O,
    O, X, X,
    O, O, X,
    X, X, O,
];

const SIX: [u16; 15] = [
    O, X, X,
    X, O, O,
    X, X, X,
    X, O, X,
    X, X, X,
];

const SEVEN: [u16; 15] = [
    X, X, X,
    O, O, X,
    O, X, O,
    O, X, O,
    O, X, O,
];

const EIGHT: [u16; 15] = [
    X, X, X,
    X, O, X,
    X, X, X,
    X, O, X,
    X, X, X,
];

const NINE: [u16; 15] = [
    X, X, O,
    X, O, X,
    X, X, X,
    O, O, X,
    X, X, O,
];

const DIGITS: [[u16;15]; 10] = [ZERO, ONE, TWO, THREE, FOUR, FIVE, SIX, SEVEN, EIGHT, NINE];

const TEST_FRAME: [u16; 300] = [
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, X, X, X, O, X, X, X, O, O, X, X, O, X, X, X, O, O, O,
    O, O, O, X, O, O, X, O, O, O, X, O, O, O, O, X, O, O, O, O,
    O, O, O, X, O, O, X, X, O, O, O, X, O, O, O, X, O, O, O, O,
    O, O, O, X, O, O, X, O, O, O, O, O, X, O, O, X, O, O, O, O,
    O, O, O, X, O, O, X, X, X, O, X, X, O, O, O, X, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
];

const BG_FRAME: [u16; 300] = [
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, X, X, X, O, X, X, X, O, O, X, X, O, X, X, X, O, O, O,
    O, O, O, X, O, O, X, O, O, O, X, O, O, O, O, X, O, O, O, O,
    O, O, O, X, O, O, X, X, O, O, O, X, O, O, O, X, O, O, O, O,
    O, O, O, X, O, O, X, O, O, O, O, O, X, O, O, X, O, O, O, O,
    O, O, O, X, O, O, X, X, X, O, X, X, O, O, O, X, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, X, X, O, O, O, X, X, O, O, O, O, O, O,
    O, O, O, O, O, O, O, X, O, X, O, X, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, X, X, O, O, X, O, X, O, O, O, O, O, O,
    O, O, O, O, O, O, O, X, O, X, O, X, O, X, O, O, O, O, O, O,
    O, O, O, O, O, O, O, X, X, O, O, O, X, X, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
];

fn paste_digit(frame: &mut [u16; 300], digit: &[u16; 15], x: usize, y: usize) {
    let stride = 20;
    let x = y * stride + x;
    frame[x] = digit[0];
    frame[x + 1] = digit[1];
    frame[x + 2] = digit[2];
    frame[x + stride] = digit[3];
    frame[x + stride + 1] = digit[4];
    frame[x + stride + 2] = digit[5];
    frame[x + (stride * 2)] = digit[6];
    frame[x + (stride * 2) + 1] = digit[7];
    frame[x + (stride * 2) + 2] = digit[8];
    frame[x + (stride * 3)] = digit[9];
    frame[x + (stride * 3) + 1] = digit[10];
    frame[x + (stride * 3) + 2] = digit[11];
    frame[x + (stride * 4)] = digit[12];
    frame[x + (stride * 4) + 1] = digit[13];
    frame[x + (stride * 4) + 2] = digit[14];
}

fn set_number(frame: &[u16; 300], number: u32) -> [u16; 300] {
    let mut output = frame.clone();
    if number < 10 {
        paste_digit(&mut output, &DIGITS[number as usize], 8, 8);
    } else if number < 100 {
        let right = number % 10;
        let left = number / 10;
        paste_digit(&mut output, &DIGITS[left as usize], 6, 8);
        paste_digit(&mut output, &DIGITS[right as usize], 10, 8);
    } else if number < 1000 {
        let right = number % 10;
        let mid = number / 10 % 10;
        let left = number / 100;
        paste_digit(&mut output, &DIGITS[left as usize], 4, 8);
        paste_digit(&mut output, &DIGITS[mid as usize], 8, 8);
        paste_digit(&mut output, &DIGITS[right as usize], 12, 8);
    }
    output
}

#[wasm_bindgen(js_name = createTestCptvFile)]
pub fn create_test_cptv_file(params: JsValue) -> Uint8Array {
    init_console();
    // Get all the things we can from params, and create a file.
    let width = 20;
    let height = 15;

    let duration_seconds = Reflect::get(&params, &JsValue::from_str("duration"))
        .unwrap_or(JsValue::from_f64(10.0))
        .as_f64()
        .unwrap();

    // Assume that duration is a positive integer number of seconds for the purposes of generating
    // test files.
    let duration_seconds = duration_seconds.round() as usize;

    // NOTE: All of these unwraps on variables are "safe" because we're setting defaults for these params
    //  in the JS calling layer
    let has_background_frame = Reflect::get(&params, &JsValue::from_str("hasBackgroundFrame"))
        .unwrap_or(JsValue::from_bool(false))
        .as_bool()
        .unwrap();

    let recording_date_time = Reflect::get(&params, &JsValue::from_str("recordingDateTime"))
        .expect("should have recordingDateTime field")
        .as_string()
        .unwrap();

    let recording_date_time =
        DateTime::parse_from_rfc3339(&recording_date_time).expect("Date parse error");

    let brand = Reflect::get(&params, &JsValue::from_str("brand"))
        .unwrap()
        .as_string();

    let model = Reflect::get(&params, &JsValue::from_str("model"))
        .unwrap()
        .as_string();

    let device_id = Reflect::get(&params, &JsValue::from_str("deviceId"))
        .unwrap()
        .as_f64()
        .map(|x| x as u32);

    let serial_number = Reflect::get(&params, &JsValue::from_str("serialNumber"))
        .unwrap()
        .as_f64()
        .map(|x| x as u32);

    let firmware_version = Reflect::get(&params, &JsValue::from_str("firmwareVersion"))
        .unwrap()
        .as_string();

    let motion_config = Reflect::get(&params, &JsValue::from_str("motionConfig"))
        .unwrap()
        .as_string();

    let preview_secs = Reflect::get(&params, &JsValue::from_str("previewSecs"))
        .unwrap()
        .as_f64()
        .map(|x| x as u8);

    let latitude = Reflect::get(&params, &JsValue::from_str("latitude"))
        .unwrap()
        .as_f64()
        .map(|x| x as f32);

    let longitude = Reflect::get(&params, &JsValue::from_str("longitude"))
        .unwrap()
        .as_f64()
        .map(|x| x as f32);

    let fps = Reflect::get(&params, &JsValue::from_str("fps"))
        .unwrap()
        .as_f64()
        .map_or(1, |x| x as u8);

    let header = CptvHeader::V2(Cptv2Header {
        timestamp: (recording_date_time.timestamp() * 1000 * 1000) as u64,
        width,
        height,
        compression: 0,
        device_name: "Test device".to_string(),
        fps,
        brand,
        model,
        device_id,
        serial_number,
        firmware_version,
        motion_config,
        preview_secs,
        latitude,
        longitude,
        loc_timestamp: None,
        altitude: None,
        accuracy: None,
        has_background_frame,
    });

    let num_header_fields = &mut 0;
    let mut output: Vec<u8> = Vec::new();
    push_header(&mut output, &header);
    let mut packed_frame_data = Vec::new();

    let mut delta_encoded_frames: Vec<(Vec<i32>, u8)> = Vec::new();
    let mut all_frames: Vec<CptvFrame> = Vec::new();

    /*
    if let V2(header) = header {
        if header.has_background_frame {
            info!("Writing background frame");
            let mut background_frame = CptvFrame::new_with_dimensions(20, 15);
            background_frame.is_background_frame = true;
            background_frame.image_data = FrameData::with_dimensions_and_data(20, 15, &BG_FRAME);
            delta_encoded_frames.push(delta_encode_frame(all_frames.last(), &background_frame));
            all_frames.push(background_frame);
        }
    }
    */

    // TODO: Can we serve files as mimetype gzipped, and have the browser do the streaming decode, and then
    //  our decoder does less work in that instance?

    // TODO: Can we write files out with an uncompressed gzip block at the start to store min/max/framecount data?
    // TODO: Zstd instead of gzip, indicated by compression flag.
    // TODO: Both gzip or Zstd can have restart blocks, and we can store a TOC in the header about them.
    // TODO: Add a flag per frame that indicates the prediction model, for backwards compat?

    // + 1 because a 1 second recording still needs two frames, a start and an end - or does it?
    // shouldn't we be able to have a single frame that we hold for 1 second?
         /*
    for frame_num in 0..=(duration_seconds * fps as usize) {
        info!("Writing frame #{}", frame_num);
        let mut test_frame = CptvFrame::new_with_dimensions(20, 15);
        test_frame.image_data = FrameData::with_dimensions_and_data(20, 15, &set_number(&TEST_FRAME, frame_num as u32));
        test_frame.time_on = (10000u32 + (frame_num as u32 * 1000u32));

        // Last FFC time was 10 seconds before test video start, so we don't have to worry about it right now.
        test_frame.last_ffc_time = Some(10u32);
        delta_encoded_frames.push(delta_encode_frame(all_frames.last(), &test_frame));
        all_frames.push(test_frame);
    }
    for (frame_num, ((delta_encode_frame, bits_per_pixel), frame)) in delta_encoded_frames
        .iter()
        .zip(all_frames.iter())
        .enumerate()
    {
        pack_frame(&mut packed_frame_data, frame, delta_encode_frame.clone(), *bits_per_pixel); // TODO: Could possibly be 4 or less for test files?
    }
    */

    output.extend_from_slice(&packed_frame_data);

    let mut buffer = Vec::new();
    {
        let mut encoder = GzEncoder::new(&mut buffer, flate2::Compression::default());
        encoder.write_all(&output).unwrap();
    }

    info!("Wrote file with length {}", buffer.len());

    unsafe { Uint8Array::view(&buffer) }
}

pub fn push_header(output: &mut Vec<u8>, cptv_header: &CptvHeader) {
    match cptv_header {
      V2(header) => {
          let num_header_fields = &mut 0;
          output.extend_from_slice(&b"CPTV"[..]);
          output.push(2);
          output.push(b'H');
          output.push(*num_header_fields);
          let header_fields_pos = output.len() - 1;


          push_field(
              output,
              &header.timestamp,
              FieldType::Timestamp,
              num_header_fields,
          );
          push_field(
              output,
              &header.width,
              FieldType::Width,
              num_header_fields,
          );
          push_field(
              output,
              &header.height,
              FieldType::Height,
              num_header_fields,
          );
          push_field(
              output,
              &header.compression,
              FieldType::Compression,
              num_header_fields,
          );
          push_field(
              output,
              &header.fps,
              FieldType::FrameRate,
              num_header_fields,
          );
          push_string(
              output,
              &header.device_name,
              FieldType::DeviceName,
              num_header_fields,
          );

          if let Some(brand) = &header.brand {
              push_string(
                  output,
                  &brand,
                  FieldType::Brand,
                  num_header_fields,
              );
          }

          if let Some(model) = &header.model {
              push_string(
                  output,
                  &model,
                  FieldType::Model,
                  num_header_fields,
              );
          }

          if let Some(device_id) = header.device_id {
              push_field(
                  output,
                  &device_id,
                  FieldType::DeviceID,
                  num_header_fields,
              );
          }

          if let Some(serial_number) = header.serial_number {
              push_field(
                  output,
                  &serial_number,
                  FieldType::CameraSerial,
                  num_header_fields,
              );
          }

          if let Some(firmware_version) = &header.firmware_version {
              push_string(
                  output,
                  &firmware_version,
                  FieldType::FirmwareVersion,
                  num_header_fields,
              );
          }

          if let Some(motion_config) = &header.motion_config {
              push_string(
                  output,
                  motion_config,
                  FieldType::MotionConfig,
                  num_header_fields,
              );
          }
          if let Some(preview_secs) = &header.preview_secs {
              push_field(
                  output,
                  preview_secs,
                  FieldType::PreviewSecs,
                  num_header_fields,
              );
          }
          if let Some(latitude) = &header.latitude {
              push_field(
                  output,
                  latitude,
                  FieldType::Latitude,
                  num_header_fields,
              );
          }
          if let Some(longitude) = &header.longitude {
              push_field(
                  output,
                  longitude,
                  FieldType::Longitude,
                  num_header_fields,
              );
          }
          if let Some(loc_timestamp) = &header.loc_timestamp {
              push_field(
                  output,
                  loc_timestamp,
                  FieldType::LocTimestamp,
                  num_header_fields,
              );
          }
          if let Some(altitude) = &header.altitude {
              push_field(
                  output,
                  altitude,
                  FieldType::Altitude,
                  num_header_fields,
              );
          }
          if let Some(accuracy) = &header.accuracy {
              push_field(
                  output,
                  accuracy,
                  FieldType::Accuracy,
                  num_header_fields,
              );
          }
          if header.has_background_frame {
              push_field(
                  output,
                  &header.has_background_frame,
                  FieldType::BackgroundFrame,
                  num_header_fields,
              );
          }
          output[header_fields_pos] = *num_header_fields;
      }
      _ => unimplemented!()
    }
}

pub fn push_frame(output: &mut Vec<u8>, frame: &CptvFrame, prev_frame: Option<&CptvFrame>, bit_widths: &mut [i32; 2], scratch: &mut [i32]) {
    let bits_per_pixel = delta_encode_frame(prev_frame, frame, scratch);
    pack_frame(output, frame, scratch, bits_per_pixel);
    if bits_per_pixel == 8 {
        bit_widths[0] += 1;
    } else {
        bit_widths[1] += 1;
    }
}

fn push_field<T: Sized>(output: &mut Vec<u8>, value: &T, code: FieldType, count: &mut u8) -> usize {
    let size = std::mem::size_of_val(value);
    output.push(size as u8);
    output.push(code as u8);
    let value_offset = output.len();
    output.extend_from_slice(unsafe {
        std::slice::from_raw_parts(value as *const T as *const u8, size)
    });
    *count += 1;
    value_offset
}

fn init_console() {
    console_error_panic_hook::set_once();
    let _ = match console_log::init_with_level(Level::Info) {
        Ok(x) => x,
        Err(_) => (),
    };
}

fn delta_encode_frame(prev_frame: Option<&CptvFrame>, frame: &CptvFrame, output: &mut [i32]) -> u8 {
    delta_encode_frame_data(
        prev_frame.map(|frame| frame.image_data.data()),
        frame.image_data.data(),
        output,
        frame.image_data.width(),
        frame.image_data.height()
    )
}

fn delta_encode_frame_data(prev_frame: Option<&[u16]>, curr: &[u16], output: &mut [i32], width: usize, height: usize) -> u8 {
    // We need to work out after the delta encoding what the range is, and how many bits we can pack
    // this into.

    // Here we are doing intra-frame delta encoding between this frame and the previous frame if
    // present.

    // FIXME - Rather than collecting here, we should be able to stream this out to the next step
    //  for large speed gains.  Or just use a scratch buffer that gets reused.
    let mut iter = (0..width)
        .chain((0..width).rev())
        .cycle()
        .take(width * height)
        .enumerate()
        .map(|(index, i)| (index, ((index / width) * width) + i));
    let mut max: i32 = 0;
    let mut prev_val = 0;


    if let Some(prev_frame) = prev_frame {
        let prev = prev_frame;

        if let Some((output_index, input_index)) = iter.next() {
            // NOTE: We can ignore the first pixel when working out our range, since that is always a literal u32
            let val = unsafe { *curr.get_unchecked(input_index) as i32 - *prev.get_unchecked(input_index) as i32 };
            let delta = val - prev_val;
            unsafe { *output.get_unchecked_mut(output_index) = delta }
            prev_val = val;
        }

        // Iterate through the remaining pixels
        for (output_index, input_index) in iter {
            let val = unsafe { *curr.get_unchecked(input_index) as i32 - *prev.get_unchecked(input_index) as i32 };
            let delta = val - prev_val;
            unsafe { *output.get_unchecked_mut(output_index) = delta }
            max = delta.abs().max(max);
            prev_val = val;
        }
    } else {
        if let Some((output_index, input_index)) = iter.next() {
            // NOTE: We can ignore the first pixel when working out our range, since that is always a literal u32
            let val = unsafe { *curr.get_unchecked(input_index) as i32 };
            let delta = val - prev_val;
            unsafe { *output.get_unchecked_mut(output_index) = delta }
            max = delta.abs().max(max);
            prev_val = val;
        }
        // Iterate through the remaining pixels
        for (output_index, input_input) in iter {
            let val = unsafe { *curr.get_unchecked(input_input) as i32 };
            let delta = val - prev_val;
            unsafe { *output.get_unchecked_mut(output_index) = delta }
            max = delta.abs().max(max);
            prev_val = val;
        }
    }
    // Now we pack into either 8 or 16 bits, depending on the range present in the frame

    // NOTE: If we go from 65535 to 0 in one step, that's a delta of -65535 which doesn't fit into 16 bits.
    //  Can this happen ever with real input?  How should we guard against it?
    //  Are there more realistic scenarios which don't work?  Let's get a bunch of lepton 3.5 files
    //  and work out the ranges there.\

    // NOTE: To play nice with lz77, we only want to pack to bytes
    let mut bits_per_pixel = (((std::mem::size_of::<i32>() as u32 * 8) - max.leading_zeros()) as u8 + 1); // Allow for sign bit
    if bits_per_pixel >= 8 {
        bits_per_pixel = 16
    } else {
        bits_per_pixel = 8
    };
    bits_per_pixel
}

fn pack_frame(
    frame_bytes: &mut Vec<u8>,
    frame: &CptvFrame,
    delta_encoded_frame: &[i32],
    bits_per_pixel: u8
) {
    let num_frame_header_fields = &mut 0;
    // Write the frame header
    frame_bytes.push(b'F');
    frame_bytes.push(*num_frame_header_fields);
    let field_count_pos = frame_bytes.len() - 1;
    let frame_size: u32 = 0;

    let frame_size_offset = push_field(
        frame_bytes,
        &frame_size,
        FieldType::FrameSize,
        num_frame_header_fields,
    );
    push_field(
        frame_bytes,
        &bits_per_pixel,
        FieldType::BitsPerPixel,
        num_frame_header_fields,
    );

    push_field(
        frame_bytes,
        &frame.time_on,
        FieldType::TimeOn,
        num_frame_header_fields,
    );
    if let Some(last_ffc_time) = frame.last_ffc_time {
        push_field(
            frame_bytes,
            &last_ffc_time,
            FieldType::LastFfcTime,
            num_frame_header_fields,
        );
    }
    // This seems problematic for our player?
    if frame.is_background_frame {
        push_field(
            frame_bytes,
            &frame.is_background_frame,
            FieldType::BackgroundFrame,
            num_frame_header_fields,
        );
    }
    frame_bytes[field_count_pos] = *num_frame_header_fields;
    // Push the first px as u32, which should (maybe) be aligned?
    let frame_data_start_offset = frame_bytes.len();

    let first_px = delta_encoded_frame[0] as u32;
    frame_bytes.push(((first_px & 0x000000ff) >> 0) as u8);
    frame_bytes.push(((first_px & 0x0000ff00) >> 8) as u8);
    frame_bytes.push(((first_px & 0x00ff0000) >> 16) as u8);
    frame_bytes.push(((first_px & 0xff000000) >> 24) as u8);

    //dbg!(bits_per_pixel);
    //pack_bits(&delta_encoded_frame[1..], frame_bytes, bits_per_pixel);
    pack_bits_fast(&delta_encoded_frame[1..], frame_bytes, bits_per_pixel);
    // Insert the frame size after it is written, including an additional 4 bytes
    let data_section_length = frame_bytes.len() - frame_data_start_offset;

    frame_bytes[frame_size_offset + 0] = ((data_section_length & 0x000000ff) >> 0) as u8;
    frame_bytes[frame_size_offset + 1] = ((data_section_length & 0x0000ff00) >> 8) as u8;
    frame_bytes[frame_size_offset + 2] = ((data_section_length & 0x00ff0000) >> 16) as u8;
    frame_bytes[frame_size_offset + 3] = ((data_section_length & 0xff000000) >> 24) as u8;
}

pub fn get_packed_frame_data(prev: Option<&[u8]>, next: &[u8], width: usize, height: usize) -> (u8, Vec<u8>) {
    let mut delta_encoded_frame = vec![0; width * height];
    let prev: Option<&[u16]> = prev.map(|bytes| unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u16, bytes.len() / 2) });
    let next = unsafe { std::slice::from_raw_parts(next.as_ptr() as *const u16, next.len() / 2) };
    let bits_per_pixel = delta_encode_frame_data(prev, next, &mut delta_encoded_frame, width, height);
    let mut output = Vec::new();
    let first_px = delta_encoded_frame[0] as u32;
    output.push(((first_px & 0x000000ff) >> 0) as u8);
    output.push(((first_px & 0x0000ff00) >> 8) as u8);
    output.push(((first_px & 0x00ff0000) >> 16) as u8);
    output.push(((first_px & 0xff000000) >> 24) as u8);
    let bits_per_pixel = 16;
    pack_bits_fast(&delta_encoded_frame[1..], &mut output, bits_per_pixel);
    (bits_per_pixel, output)
}


fn pack_bits_fast(input: &[i32], frame_bytes: &mut Vec<u8>, width: u8) {
    if width == 8 {
        for px in input {
            frame_bytes.push(*px as u8);
        }
    } else if width == 16 {
        for px in input {
            let px = *px as u16;
            frame_bytes.push((px >> 8) as u8);
            frame_bytes.push(px as u8);
        }
    }
}


#[inline(always)]
fn twos_comp(v: i32, width: u8) -> u32 {
    if v >= 0 {
        v as u32
    } else {
        !(-v as u32) + 1 & ((1<<width) - 1) as u32
    }
}
// For use if we want to pack bits to arbitrary packing widths
fn pack_bits(input: &[i32], frame_bytes: &mut Vec<u8>, width: u8) {
    let mut scratch = 0;
    let mut n = 0u8;
    for px in input {
        scratch |= twos_comp(*px, width) << (32 - width - n);
        n += width;
        while n >= 8 {
            frame_bytes.push((scratch >> 24) as u8);
            scratch <<= 8;
            n -= 8;
        }
    }
    if n > 0 {
        frame_bytes.push((scratch >> 24) as u8);
    }
}

fn push_string(output: &mut Vec<u8>, value: &str, code: FieldType, count: &mut u8) {
    output.push(value.len() as u8);
    output.push(code as u8);
    output.extend_from_slice(value.as_bytes());
    *count += 1;
}
