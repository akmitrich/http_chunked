use anyhow::{Context, Ok};

use super::{get_line, skip_line};

#[derive(Debug, Clone, PartialEq)]
pub enum HttpHeader {
    Custom { name: String, value: String },
    ContentLength(usize),
    ContentType { media_type: String },
}

impl HttpHeader {
    pub fn from_name_value(name: &str, value: &str) -> anyhow::Result<Self> {
        match name.to_lowercase().as_str() {
            "content-length" => Ok(Self::ContentLength(
                value.trim().parse().context("parse content length")?,
            )),
            "content-type" => Ok(Self::ContentType {
                media_type: value.trim().to_owned(),
            }),
            _ => Ok(Self::Custom {
                name: name.to_owned(),
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Debug)]
pub struct HeaderIter<'a> {
    cursor: &'a [u8],
}

impl<'a> Iterator for HeaderIter<'a> {
    type Item = HttpHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.is_empty() {
            return None;
        }
        let result = parse_header(self.cursor).ok()?;
        self.cursor = skip_line(self.cursor);
        Some(result)
    }
}

impl<'a> HeaderIter<'a> {
    pub fn new(cursor: &'a [u8]) -> Self {
        Self { cursor }
    }
}

fn parse_header(line: &[u8]) -> anyhow::Result<HttpHeader> {
    let (name, value) = std::str::from_utf8(get_line(line))
        .context("parse header with non-UTF8")?
        .split_once(':')
        .ok_or_else(|| anyhow::Error::msg("semicolon ':' not found"))?;
    HttpHeader::from_name_value(name.trim(), value)
}
