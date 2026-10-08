pub struct Response {
    status: StatusLine,
}

pub struct StatusLine {
    version: Version,
    code: StatusCode,
    text: StatusText,

}

pub enum Version {
    HTTP1_1,
}

pub struct StatusCode {
}

pub struct StatusText {
}
