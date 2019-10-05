use derivative::Derivative;
use libflate::gzip::Decoder;
use nom::number::complete::{le_f32, le_u32, le_u64, le_u8};
use nom::{bytes::complete::*, Err};
use std::fmt::{Error, Formatter};
use std::fs::File;
use std::io::{BufWriter, Read};
use std::ops::{Index, IndexMut};
use std::path::Path;
use std::{fmt, fs};

fn main() -> std::result::Result<(), std::boxed::Box<dyn std::error::Error>> {
    // 20190922-021028
    // 20190922-021916
    match fs::read("20190922-021916.cptv") {
        Ok(input) => {
            let mut gz_decoder = Decoder::new(&input[..])?;
            let mut decoded = Vec::new();
            gz_decoder.read_to_end(&mut decoded)?;
            match decode_cptv(&decoded) {
                Ok((_, cptv)) => {
                    //dump_png_frames(&cptv);
                    try_compression(&cptv);
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

#[derive(Debug)]
struct CptvHeader {
    timestamp: u64,
    width: u32,
    height: u32,
    compression: u8,
    device_name: String,

    motion_config: Option<String>,
    preview_secs: Option<u8>,
    latitude: Option<f32>,
    longitude: Option<f32>,
    loc_timestamp: Option<u64>,
    altitude: Option<f32>,
    accuracy: Option<f32>,
}

#[derive(Clone, Copy)]
struct FrameData([[i16; 160]; 120]);

impl FrameData {
    pub fn empty() -> FrameData {
        FrameData([[0i16; 160]; 120])
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                &self[0] as *const i16 as *const u8,
                std::mem::size_of_val(self),
            )
        }
    }
}

impl Index<usize> for FrameData {
    type Output = [i16; 160];

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for FrameData {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
struct CptvFrame {
    time_on: u32,
    bit_width: u8,
    frame_size: u32,
    last_ffc_time: u32,
    #[derivative(Debug = "ignore")]
    image_data: FrameData,
}

struct Cptv {
    meta: CptvHeader,
    frames: Vec<CptvFrame>,
}

impl fmt::Debug for Cptv {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "CPTV file with {} frames", self.frames.len())
    }
}

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
    frame.image_data[0][0] = (prev_px as i32 + current_px) as i16;
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
        frame.image_data[y][x] = (prev_px as i32 + current_px) as i16;
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
        image_data: FrameData::empty(),
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
            x => panic!("Unknown frame field type {} {:?}", x, frame),
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
    Ok((i, frames))
}

fn decode_cptv(i: &[u8]) -> nom::IResult<&[u8], Cptv> {
    // For reading and opening files
    let (i, meta) = decode_header(i)?;
    let (i, frames) = decode_frames(i, meta.width, meta.height)?;
    assert_eq!(i.len(), 0);
    Ok((i, Cptv { frames, meta }))
}

fn dump_png_frames(cptv: &Cptv) {
    // Work out the dynamic range to scale here:
    let mut min = std::i16::MAX;
    let mut max = 0;
    for frame in &cptv.frames {
        for y in 0..cptv.meta.height as usize {
            for x in 0..cptv.meta.width as usize {
                let val = frame.image_data[y][x];
                min = i16::min(val, min);
                max = i16::max(val, max);
            }
        }
    }
    // Can we work out the greatest delta between any two adjacent pixels?
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

fn try_compression(cptv: &Cptv) {
    let mut frames_size = 0;
    let i_frame_interval = 9 * 5;
    let mut delta_frames = Vec::new();
    for (frame_index, frames) in cptv.frames.windows(2).enumerate() {
        let is_first_frame = frame_index == 0;
        let is_i_frame = frame_index % i_frame_interval == 0;
        let frame_index = frame_index + 1;

        let frame_a = &frames[0];
        let frame_b = &frames[1];

        // Delta between frames, then in frame?
        if is_first_frame {
            delta_frames.push(delta_compress_frame(&frame_a.image_data));
        } else if is_i_frame {
            delta_frames.push(delta_compress_frame(&frame_b.image_data));
        } else {
            let mut d: FrameData = FrameData::empty();
            for y in 0..cptv.meta.height as usize {
                for x in 0..cptv.meta.width as usize {
                    d[y][x] = frame_b.image_data[y][x] - frame_a.image_data[y][x];
                }
            }
            delta_frames.push(delta_compress_frame(&d));
        }
        /*
        let side = 4;
        let mut y = 0;
        let mut x = 0;
        while y < cptv.meta.height as usize {
            while x < cptv.meta.height as usize {
                // Delta encode a 4x4 block in 2 passes:
                for i in 0..side {
                    let x = x + i;
                    d[y + 0][x] = d[y + 0][x];
                    d[y + 1][x] = d[y + 1][x] - d[y + 0][x];
                    d[y + 2][x] = d[y + 2][x] - d[y + 1][x];
                    d[y + 3][x] = d[y + 3][x] - d[y + 2][x];
                }
                x += side;
            }
            y += side;
        }
        let mut y = 0;
        let mut x = 0;
        while y < cptv.meta.height as usize {
            while x < cptv.meta.height as usize {
                // Delta encode a 4x4 block in 2 passes:
                for i in 0..side {
                    let y = y + i;
                    d[y][x + 0] = d[y][x + 0];
                    d[y][x + 1] = d[y][x + 1] - d[y][x + 0];
                    d[y][x + 2] = d[y][x + 2] - d[y][x + 1];
                    d[y][x + 3] = d[y][x + 3] - d[y][x + 2];
                }
                x += side;
            }
            y += side;
        }
        */
    }
    // IDEA: Since we are only making it so you can go to the beginning of each iframe to start
    //  decode, we should also make the subsequent frames up until the next iframe part of the zstd
    //  compression, for additional size reductions.
    for (index, frame) in delta_frames.iter().enumerate() {
        let compressed = zstd::encode_all(frame.as_slice(), 9);
        if let Ok(compressed) = compressed {
            frames_size += compressed.len();
            println!("Zstd frame: {}", compressed.len());
        }
    }
    println!("All frames {}", frames_size);
}

fn delta_compress_frame(data: &FrameData) -> FrameData {
    data.clone()
}
