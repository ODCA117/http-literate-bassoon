#[derive(Debug)]
pub struct Header<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
}
