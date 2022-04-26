use std::borrow::Borrow;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use nom::bytes::streaming::{tag, take};
use nom::number::streaming::le_u8;
use cptv_decoder::decoder;
use cptv_encoder::{push_frame, push_header};
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::{Compression, Decompress, FlushDecompress};
use std::io;
use std::io::prelude::*;

use cptv_shared::v2::{decode_cptv2_header, decode_frame_header_v2, unpack_frame_v2};

#[cfg(feature = "cptv3-support")]
use cptv_shared::v3::decode_cptv3_header;

use cptv_shared::CptvHeader;
use cptv_shared::CptvHeader::V2;
use cptv_shared::v2::types::CptvFrame;

use walkdir::WalkDir;

fn main() -> std::io::Result<()> {

    // TODO: Take a folder as args from the CLI

    // TODO - par_iter over files.

    // Load cptv:
    // Need a synchronous version of the decoder first?
    // TODO: Streaming decoder?
    let path = if Path::new(&"../cptv-files").exists() {
        "../cptv-files"
    } else {
        "./cptv-files"
    };
    let src_dir = WalkDir::new("./");
    let mut max = 0;
    for entry in src_dir {
        if max == 10 {
            break;
        }
        if let Ok(entry) = entry {
            if let Some(ext) = entry.path().extension() {
                if ext == "cptv" {
                    check_cptv(entry.path());
                    max += 1;
                }
            }
        }
    }
    Ok(())
}


fn check_cptv(path: &Path) -> std::io::Result<()> {
    let (original_size, (header, frames)) = decode_file(path)?;
    if let V2(ref v2_header) = header {
        let start = Instant::now();
        // Compress cptv:
        let mut output = Vec::with_capacity(v2_header.width as usize * v2_header.height as usize * 16 * frames.len());
        push_header(&mut output, &header);
        debug_assert_ne!(frames.len(), 0);
        // Add the first frame

        let mut scratch = vec![0; v2_header.width as usize * v2_header.height as usize];
        let mut bit_widths = [0, 0];
        push_frame(&mut output, &frames[0], None, &mut bit_widths, &mut scratch);
        // TODO: When array_windows is stabilised, use that instead.
        for (frame_num, window) in frames.windows(2).enumerate() {
            match window {
                [prev, next] => push_frame(&mut output, next, Some(prev), &mut bit_widths, &mut scratch),
                _ => panic!("This shouldn't happen")
            }
        }

        //println!("8's {}, 16's {}", bit_widths[0], bit_widths[1]);

        //println!("Packing took {:?}", Instant::now().duration_since(start));
        let start_gz = Instant::now();

        // TODO: Reuse this buffer?
        let mut buffer = Vec::with_capacity(v2_header.width as usize * v2_header.height as usize * 16);

        // // TODO - stream *into* gzip, leaving the header section uncompressed.
        {
            let mut encoder = GzEncoder::new(&mut buffer, Compression::default());
            encoder.write_all(&output).unwrap();
        }
        let end = Instant::now();

        //std::fs::write("../cptv-files/out.cptv", &buffer);
        println!("==== {:?}", &path);
        println!("Frames: {}, Model: {:?}, original size {}, new size {}, savings {}x", frames.len(), v2_header.model, original_size, buffer.len(), original_size as f32 / buffer.len() as f32);
        println!("Timings: packing {:?}, gz {:?}, total {:?} pack fps {}, fps total {}",
                 start_gz.duration_since(start),
                 end.duration_since(start_gz),
                 end.duration_since(start),
                 frames.len() as f64 / (start_gz.duration_since(start)).as_secs_f64(),
                 frames.len() as f64 / (end.duration_since(start)).as_secs_f64()
        );
        // TODO: Test with arbitrary 16bit noise

        // let (original_size, (header2, frames2)) = decode_buffer(&output)?;
        //
        // if let V2(header2) = header2 {
        //     assert_eq!(v2_header.width, header2.width);
        //     assert_eq!(v2_header.height, header2.height);
        //     assert_eq!(frames.len(), frames2.len());
        //     // Make sure this is equal to the original input (once decoded).
        //     for (frame_num, (original, new)) in frames.iter().zip(frames2.iter()).enumerate() {
        //         for y in 0..header2.height as usize {
        //             for x in 0..header2.width as usize {
        //                 let a = original.image_data[y][x];
        //                 let b = new.image_data[y][x];
        //                 assert_eq!(a, b, "Failed equality test on frame #{}, @{},{} {}/{}", frame_num, x, y, original.bit_width, new.bit_width);
        //             }
        //         }
        //         // for (i, (a, b)) in original.image_data.data().iter().zip(new.image_data.data().iter()).enumerate() {
        //         //     assert_eq!(*a, *b, "Failed equality test on frame #{}, px {} {}/{}", frame_num, i, original.bit_width, new.bit_width);
        //         // }
        //     }
        // }
    }
    Ok(())
}


fn decode_buffer(buffer: &Vec<u8>) -> Result<(CptvHeader, Vec<CptvFrame>), Error> {
    if let Ok((mut body, header)) = decoder::decode_cptv_header(buffer) {
        match header {
            V2(header) => {
                let mut frames = Vec::new();
                while let Ok((rest, (image, mut frame))) = decode_frame_header_v2(body, header.width as usize, header.height as usize) {
                    let last = frames.pop();
                    unpack_frame_v2(&last, image, &mut frame);
                    if let Some(last) = last {
                        frames.push(last);
                    }
                    frames.push(frame);
                    body = rest;
                }
                Ok((CptvHeader::V2(header), frames))
            }
            _ => unimplemented!()
        }
    } else {
        Err(Error::new(ErrorKind::Other, "Oops"))
    }
}

fn decode_file(file_path: &Path) -> Result<(usize, (CptvHeader, Vec<CptvFrame>)), Error> {
    let mut file = File::open(file_path)?;
    let mut raw_buffer = Vec::new();
    file.read_to_end(&mut raw_buffer)?;
    let mut buffer = Vec::new();
    let start = Instant::now();
    let mut decoder = GzDecoder::new(&*raw_buffer);
    decoder.read_to_end(&mut buffer);
    //println!("Unzip took {:?}", Instant::now().duration_since(start));
    let result = decode_buffer(&buffer)?;
    Ok((raw_buffer.len(), result))
}
