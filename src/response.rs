use core::str::from_utf8;

use crate::{
    header::{Header, parse_headers},
    http_common::{HttpParseError, Version},
};
use heapless::Vec;
use nom::{
    IResult, Parser,
    bytes::complete::{tag, take_till, take_until},
    character::complete::{multispace0, u8 as nom_u8},
    combinator::map_res,
    sequence::{preceded, terminated},
};

#[derive(Debug)]
pub struct Response<'a> {
    status: StatusLine<'a>,
    headers: Vec<Header<'a>, 32>,
    body: &'a [u8],
}

#[derive(Debug)]
pub struct StatusLine<'a> {
    version: Version,
    code: StatusCode,
    text: StatusText<'a>,
}

fn http_word(input: &[u8]) -> IResult<&[u8], &[u8]> {
    preceded(multispace0, take_till(|b: u8| b.is_ascii_whitespace())).parse(input)
}

fn http_version(input: &[u8]) -> IResult<&[u8], Version> {
    let (input, version) = http_word(input)?;
    let (rest, _) = tag("HTTP/")(version)?;
    let (rest, major) = nom_u8(rest)?;
    let (rest, _) = tag(".")(rest)?;
    let (_, minor) = nom_u8(rest)?;
    let version = Version::new(major, minor);
    Ok((input, version))
}

fn http_status_code(input: &[u8]) -> IResult<&[u8], StatusCode> {
    let (input, status_code) = http_word(input)?;
    let status_code = match status_code {
        b"200" => StatusCode::Ok200,
        b"201" => StatusCode::Created201,
        b"204" => StatusCode::NoContent204,
        b"301" => StatusCode::Redirect301,
        b"400" => StatusCode::BadRequest400,
        b"500" => StatusCode::InternalError500,
        _ => StatusCode::InternalError500,
    };

    Ok((input, status_code))
}

fn http_status_text(input: &[u8]) -> IResult<&[u8], StatusText<'_>> {
    let (input, status_text) = terminated(
        map_res(take_until("\r\n".as_bytes()), from_utf8),
        tag("\r\n".as_bytes()),
    )
    .parse(input)?;

    Ok((input, StatusText::<'_> { text: status_text }))
}

fn parse_status_line(input: &[u8]) -> IResult<&[u8], StatusLine<'_>> {
    let (input, version) = http_version(input)?;
    let (input, code) = http_status_code(input)?;
    let (input, text) = http_status_text(input)?;

    let status_line = StatusLine {
        version,
        code,
        text,
    };

    Ok((input, status_line))
}

impl<'a> Response<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<Self, HttpParseError> {
        let (mut input, status) = parse_status_line(buf)?;
        let mut headers = Vec::new();
        while !input.starts_with(b"\r\n") {
            let (rest, header) = parse_headers(input)?;
            headers
                .push(header)
                .map_err(|_| HttpParseError::TooManyHeaders)?;
            input = rest;
        }

        let (body, _) = tag("\r\n".as_bytes())(input)?;

        let resp = Response {
            status,
            headers,
            body,
        };

        Ok(resp)
    }
}

// TODO: Fix this type
#[derive(Debug, PartialEq)]
pub enum StatusCode {
    Ok200,
    Created201,
    NoContent204,
    Redirect301,
    BadRequest400,
    InternalError500,
}

#[derive(Debug)]
pub struct StatusText<'a> {
    text: &'a str,
}

#[cfg(test)]
mod tests {
    // extern crate std;
    use super::*;

    const SIMPLE_OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n";
    const OK_WITH_BODY: &[u8] =
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello";
    const CREATED: &[u8] =
        b"HTTP/1.1 201 Created\r\nLocation: /items/42\r\nContent-Length: 5\r\n\r\nid=42";
    const JSON_RESPONSE: &[u8] =
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 18\r\n\r\n{\"brightness\": 128}";
    const NO_CONTENT: &[u8] = b"HTTP/1.1 204 No Content\r\n\r\n";
    const REDIRECT: &[u8] =
        b"HTTP/1.1 301 Moved Permanently\r\nLocation: http://example.com/\r\nContent-Length: 0\r\n\r\n";
    const MANY_HEADERS: &[u8] = b"HTTP/1.1 200 OK\r\nServer: h\r\nContent-Type: text/html\r\nContent-Length: 2\r\nConnection: keep-alive\r\nX-Custom: 1\r\n\r\nok";

    // Edge cases that should FAIL parsing:
    const NO_STATUS_CODE: &[u8] = b"HTTP/1.1\r\n\r\n"; // missing status code + reason
    const BAD_VERSION: &[u8] = b"200 OK http/1.1\r\n\r\n"; // grammar violation
    const NO_CRLF: &[u8] = b"HTTP/1.1 200 OK"; // truncated
    const UNKNOWN_STATUS: &[u8] = b"HTTP/1.1 599 Teapot Overflow\r\nContent-Length: 0\r\n\r\n"; // should PARSE as Other

    #[test]
    fn test_valid_ok() {
        let buf = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
        let res = Response::parse(buf).expect("Failed to parse valid HTTP response");
        assert_eq!(res.status.version, Version::HTTP1_1);
        assert_eq!(res.status.code, StatusCode::Ok200);
        assert_eq!(res.headers.len(), 1);
        assert_eq!(res.body.len(), 0);
    }

    #[test]
    fn test_simple_ok() {
        Response::parse(SIMPLE_OK).expect("Failed to parse simple HTTP 200");
    }

    #[test]
    fn test_ok_with_body() {
        Response::parse(OK_WITH_BODY).expect("Failed HTTP 200 with body");
    }

    #[test]
    fn test_created() {
        Response::parse(CREATED).expect("Failed HTTP 201 Created");
    }

    #[test]
    fn test_json_response() {
        Response::parse(JSON_RESPONSE).expect("Failed HTTP json response");
    }

    #[test]
    fn test_no_content() {
        Response::parse(NO_CONTENT).expect("Failed HTTP 204 No Content");
    }

    #[test]
    fn test_redirect() {
        Response::parse(REDIRECT).expect("Failed HTTP 301 redirect");
    }

    #[test]
    fn test_many_headers() {
        Response::parse(MANY_HEADERS).expect("Failed HTTP response with many headers");
    }

    #[test]
    fn test_no_status_code() {
        assert!(Response::parse(NO_STATUS_CODE).is_err());
    }

    #[test]
    fn test_bad_version() {
        assert!(Response::parse(BAD_VERSION).is_err());
    }

    #[test]
    fn test_no_crlf() {
        assert!(Response::parse(NO_CRLF).is_err());
    }

    #[test]
    fn test_unknown_status() {
        Response::parse(UNKNOWN_STATUS).expect("Failed to parse unknown status code");
    }
}
