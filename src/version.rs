
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
