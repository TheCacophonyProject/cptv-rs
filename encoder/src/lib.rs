use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use cptv_shared::v2::types::{FieldType, CptvFrame, Cptv2Header};

fn push_field<T: Sized>(output: &mut Vec<u8>, value: &T, code: FieldType) -> usize {
    let size = std::mem::size_of_val(value);
    output.push(code as u8);
    output.push(size as u8);
    let value_offset = output.len();
    output.extend_from_slice(unsafe {
        std::slice::from_raw_parts(value as *const T as *const u8, size)
    });
    value_offset
}

// TODO(jon): Move these into cargo workspaces.

const X: u16 = 0u16;
const O: u16 = 1u16;
const TEST_FRAME: [u16; 300] = [
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
    O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
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

fn pack_frame(frame_bytes: &mut Vec<u8>, frame: &CptvFrame) {
    // Work out whether this frame can be easily represented in i8 space, using one byte per pixel.
    let frame_range = get_dynamic_range(&frame.image_data);
    let pixel_bytes = if frame_range.len() <= u8::MAX as usize
        && *frame_range.start() >= i8::MIN as i16
        && *frame_range.end() <= i8::MAX as i16
    {
        1u8
    } else {
        2u8
    };

    // NOTE(jon): We only want to do this if the values in the frame can all be represented as
    //  i8s without any offsetting: offsetting other values that do have a dynamic range <= 255
    //  would still skew our data away from having most of the delta values be around 0, and actually
    //  hurts compressibility, since it varies the data more between frames.

    // Write the frame header
    push_field(frame_bytes, &4u8, FieldType::FrameHeader);
    push_field(
        frame_bytes,
        &(frame.width() * frame.height() * pixel_bytes as u32),
        FieldType::FrameSize,
    );
    // NOTE(jon): Frame size is technically redundant, as it will always be width * height * pixel_bytes
    push_field(frame_bytes, &pixel_bytes, FieldType::PixelBytes);
    push_field(frame_bytes, &meta.time_on, FieldType::TimeOn);
    push_field(frame_bytes, &meta.last_ffc_time, FieldType::LastFfcTime);
    if pixel_bytes == 1 {
        // Seems fair to say that most frames fit comfortably inside 8 bits.
        for y in 0..frame.height() {
            for x in 0..frame.width() {
                let val = frame[y][x] as i8 as u8;
                frame_bytes.push(val);
            }
        }
    } else {
        frame_bytes.extend_from_slice(frame.image_data.as_slice());
    }
}

fn push_string(output: &mut Vec<u8>, value: &str, code: FieldType) {
    output.push(code as u8);
    output.push(value.len() as u8);
    output.extend_from_slice(value.as_bytes());
}

#[wasm_bindgen]
pub fn create_test_cptv_file(params: JsValue) -> Uint8Array {
    // Get all the things we can from params, and create a file.
    let width = Reflect::get(&params, &JsValue::from_str("width"))
        .expect("Should have property 'done'")
        .as_f64()
        .unwrap() as usize;
    let height = Reflect::get(&params, &JsValue::from_str("height"))
        .expect("Should have property 'done'")
        .as_f64()
        .unwrap() as usize;

    let mut meta = Cptv2Header {
        timestamp: 0,
        width: 0,
        height: 0,
        compression: 0,
        device_name: "".to_string(),
        fps: 0,
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
        has_background_frame: false,
    };

    let mut output: Vec<u8> = Vec::new();
    output.extend_from_slice(&b"CPTV"[..]);
    output.push(2);
    let mut num_fields = 0;
    let num_fields_offset = push_field(&mut output, &num_fields, FieldType::Header);
    push_field(&mut output, &meta.timestamp, FieldType::Timestamp);
    push_field(&mut output, &meta.width, FieldType::Width);
    push_field(&mut output, &meta.height, FieldType::Height);
    push_field(&mut output, &meta.compression, FieldType::Compression);
    push_field(&mut output, &meta.fps, FieldType::FrameRate);
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
    let mut frames = Vec::new();
    pack_frame(&mut frames, &CptvFrame::new_with_dimensions(width, height));
    output.extend_from_slice(&frames);

    unsafe { Uint8Array::view(&output) }
}
