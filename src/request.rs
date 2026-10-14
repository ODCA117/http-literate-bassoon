use super::header::{Header, parse_headers};
use super::http_common::{HttpParseError, Version};
use core::str::from_utf8;
use core::result::Result;
use heapless::Vec;
// TODO: convert to streaming
use nom::bytes::complete::{tag, take_till};
use nom::character::complete::u8 as nom_u8;
use nom::character::multispace0;
use nom::combinator::map_res;
use nom::sequence::preceded;
use nom::{IResult, Parser};

#[derive(Debug)]
pub struct Request<'a> {
    line: RequestLine<'a>,
    headers: Vec<Header<'a>, 32>,
    data: &'a [u8],
}

#[derive(Debug)]
pub struct RequestLine<'a> {
    method: Method<'a>,
    path: Path<'a>,
    version: Version,
}

#[derive(Debug, PartialEq)]
pub enum Method<'a> {
    GET,
    Other(&'a [u8]),
}

impl<'a> Method<'a> {
    fn new(method: &'a [u8]) -> Self {
        match method {
            b"GET" => Method::GET,
            _ => Method::Other(method),
        }
    }
}

#[derive(Debug)]
// TODO: Only prints
pub struct Path<'a> {
    path: &'a str,
}

fn http_word(input: &[u8]) -> IResult<&[u8], &[u8]> {
    preceded(multispace0(), take_till(|b: u8| b.is_ascii_whitespace())).parse(input)
}

fn http_method(input: &[u8]) -> IResult<&[u8], Method<'_>> {
    let (input, method) =
        preceded(multispace0(), take_till(|b: u8| b.is_ascii_whitespace())).parse(input)?;
    if method.iter().any(|b| b.is_ascii_lowercase()) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }

    let method = Method::new(method);
    Ok((input, method))
}

fn http_path(input: &[u8]) -> IResult<&[u8], Path<'_>> {
    let (input, path) = preceded(
        multispace0(),
        map_res(take_till(|b: u8| b.is_ascii_whitespace()), from_utf8),
    )
    .parse(input)?;
    Ok((input, Path { path }))
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

fn parse_reqline(input: &[u8]) -> IResult<&[u8], RequestLine<'_>> {
    let (input, method) = http_method(input)?;
    let (input, path) = http_path(input)?;
    let (input, version) = http_version(input)?;

    let req_line = RequestLine {
        method,
        path,
        version,
    };

    Ok((input, req_line))
}

impl<'a> Request<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<Self, HttpParseError> {
        let (input, line) = parse_reqline(buf)?;
        let (mut input, _) = tag("\r\n".as_bytes())(input)?;
        let mut headers = Vec::new();
        while !input.starts_with(b"\r\n") {
            let (rest, header) = parse_headers(input)?;
            headers
                .push(header)
                .map_err(|_| HttpParseError::TooManyHeaders)?;
            input = rest;
        }
        if !headers.iter().find(|h| h.name.eq("Host")).is_some() {
            return Err(HttpParseError::Malformed);
        }

        let (body, _) = tag("\r\n".as_bytes())(input)?;

        let req = Request {
            line,
            headers,
            data: body,
        };

        Ok(req)
    }
}

#[cfg(test)]
mod tests {
    // extern crate std;
    use super::*;

    const SIMPLE_GET: &[u8] = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
    const GET_WITH_QUERY: &[u8] =
        b"GET /search?q=rust+no_std&page=2 HTTP/1.1\r\nHost: example.com\r\n\r\n";
    const POST_FORM: &[u8] =
    b"POST /submit HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 13\r\n\r\nname=olle&x=1";
    const POST_JSON: &[u8] =
    b"POST /api/led HTTP/1.1\r\nHost: pico.local\r\nContent-Type: application/json\r\nContent-Length: 18\r\n\r\n{\"brightness\": 128}";
    const HEAD_REQUEST: &[u8] = b"HEAD /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
    const MANY_HEADERS: &[u8] = b"GET / HTTP/1.1\r\nHost: h\r\nAccept: */*\r\nAccept-Encoding: gzip\r\nUser-Agent: test\r\nConnection: keep-alive\r\nX-Custom: 1\r\n\r\n";

    // Edge cases that should FAIL parsing:
    const NO_HOST_HTTP11: &[u8] = b"GET / HTTP/1.1\r\n\r\n"; // 400 per RFC (Host required)
    const LOWERCASE_METHOD: &[u8] = b"get / HTTP/1.1\r\n\r\n"; // methods are case-sensitive → Other or reject?
    const BAD_VERSION: &[u8] = b"GET / http/1.1\r\n\r\n"; // grammar violation
    const NO_CRLF: &[u8] = b"GET / HTTP/1.1"; // truncated
    const UNKNOWN_METHOD: &[u8] = b"BREW /pot HTTP/1.1\r\nHost: h\r\n\r\n"; // should PARSE as Other

    #[test]
    fn test_valid_get() {
        let buf = b"GET /index.html HTTP/1.1\r\nHost: example.domain\r\n\r\n";
        let req = Request::parse(buf).expect("Failed to parse valid HTTP");
        assert_eq!(req.line.version, Version::HTTP1_1);
        assert_eq!(req.line.method, Method::GET);
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.data.len(), 0);
    }

    #[test]
    fn test_simple_get() {
        Request::parse(SIMPLE_GET).expect("Failed to parse simple HTTP GET");
    }

    #[test]
    fn test_get_with_query() {
        Request::parse(GET_WITH_QUERY).expect("Failed HTTP GET with Query");
    }

    #[test]
    fn test_post_form() {
        Request::parse(POST_FORM).expect("Failed HTTP POST form");
    }

    #[test]
    fn test_post_json() {
        Request::parse(POST_JSON).expect("Failed HTTP POST json");
    }

    #[test]
    fn test_head() {
        Request::parse(HEAD_REQUEST).expect("Failed HTTP HEAD");
    }

    #[test]
    fn test_many_headers() {
        Request::parse(MANY_HEADERS).expect("Failed HTTP many headers");
    }

    #[test]
    fn test_no_host() {
        assert!(Request::parse(NO_HOST_HTTP11).is_err());
    }

    #[test]
    fn test_lowercase_method() {
        assert!(Request::parse(LOWERCASE_METHOD).is_err());
    }

    #[test]
    fn test_bad_version() {
        assert!(Request::parse(BAD_VERSION).is_err());
    }

    #[test]
    fn test_no_crlf() {
        assert!(Request::parse(NO_CRLF).is_err());
    }

    #[test]
    fn test_unknow_method() {
        Request::parse(UNKNOWN_METHOD).expect("Failed to parse unknown method");
    }
}
