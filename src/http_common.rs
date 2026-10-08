#[derive(Debug)]
pub enum HttpParseError {
    Malformed,
    InvalidUtf8,
    TooManyHeaders,
}

type NomErr<'a> = nom::Err<nom::error::Error<&'a [u8]>>;

impl<'a> From<NomErr<'a>> for HttpParseError {
    fn from(e: NomErr<'a>) -> Self {
        match e {
            _ => HttpParseError::Malformed, // nom::Err::Incomplete(needed) => todo!(),
                                            // nom::Err::Error(_) => todo!(),
                                            // nom::Err::Failure(_) => todo!(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Version {
    HTTP1_1,
    Other { major: u8, minor: u8 },
}

impl Version {
    pub fn new(major: u8, minor: u8) -> Self {
        match (major, minor) {
            (1, 1) => Version::HTTP1_1,
            _ => Version::Other { major, minor },
        }
    }
}
