use cptv_common::{Cptv, CptvHeader};
use nom::bytes::complete::{tag, take};
use nom::number::complete::{le_f32, le_u32, le_u64, le_u8};

fn decode_header(i: &[u8]) -> nom::IResult<&[u8], CptvHeader> {
    let mut meta = CptvHeader {
        timestamp: 0,
        width: 0,
        height: 0,
        compression: 0,
        device_name: String::new(),
        motion_config: None,
        preview_secs: None,
        latitude: None,
        longitude: None,
        loc_timestamp: None,
        altitude: None,
        accuracy: None,
    };

    let (i, _) = tag(b"CPTV")(i)?;
    let (i, version) = le_u8(i)?;
    assert_eq!(version, 2);
    let (i, _) = tag(b"H")(i)?;
    let (i, num_header_fields) = le_u8(i)?;

    let mut outer = i;
    for _ in 0..num_header_fields as usize {
        let (i, field_length) = le_u8(outer)?;
        let (i, field) = le_u8(i)?;
        let (i, val) = take(field_length)(i)?;
        outer = i;
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
            x => panic!("Unknown header field type {} {:?}", x, meta),
        }
    }
    Ok((outer, meta))
}

pub fn decode_cptv3(input: &Vec<u8>) -> Cptv {}
