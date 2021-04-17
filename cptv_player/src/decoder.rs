use cptv_common::{Cptv2Header, Cptv3Header};
use nom::bytes::streaming::{tag, take};
use nom::error::{ErrorKind, ParseError};
use nom::number::streaming::le_u8;

#[cfg(feature = "cptv2-support")]
use crate::v2::decode_cptv2_header;

#[cfg(feature = "cptv3-support")]
use crate::v3::decode_cptv3_header;

#[derive(Debug)]
pub enum CptvHeader {
    UNINITIALISED,
    V3(Cptv3Header),
    V2(Cptv2Header),
}

pub fn decode_cptv_header(i: &[u8]) -> nom::IResult<&[u8], CptvHeader> {
    let (i, val) = take(4usize)(i)?;
    let (_, _) = tag(b"CPTV")(val)?;
    let (i, version) = le_u8(i)?;
    match version {
        2 => decode_cptv2_header(i),
        3 => {
            {
                #[cfg(feature = "cptv3-support")]
                decode_cptv3_header(i)
            }
            {
                #[cfg(not(feature = "cptv3-support"))]
                Err(nom::Err::Failure(ParseError::add_context(
                    i,
                    "cptv3 support not enabled",
                    (i, ErrorKind::Tag),
                )))
            }
        }
        _ => Err(nom::Err::Failure(ParseError::add_context(
            i,
            "Unknown CPTV version",
            (i, ErrorKind::Tag),
        ))),
    }
}
