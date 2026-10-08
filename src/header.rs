use core::str::from_utf8;

use nom::{
    IResult, Parser,
    bytes::complete::{tag, take_until},
    character::multispace0,
    combinator::map_res,
    sequence::terminated,
};

#[derive(Debug)]
pub struct Header<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
}

pub fn parse_headers(input: &[u8]) -> IResult<&[u8], Header<'_>> {
    let (rest, header) =
        terminated(take_until("\r\n".as_bytes()), tag("\r\n".as_bytes())).parse(input)?;

    // NOTE: Could have issues if space between name and colon
    let (value, name) = map_res(take_until(":".as_bytes()), from_utf8).parse(header)?;
    let (value, _) = terminated(tag(":".as_bytes()), multispace0()).parse(value)?;

    let header = Header { name, value };

    Ok((rest, header))
}
