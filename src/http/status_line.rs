use anyhow::Context;

use super::end_of_line;

#[derive(Debug, Clone, Copy)]
pub struct Status<'a> {
    code: u16,
    reason_phrase: &'a str,
}

impl<'a> Status<'a> {
    pub fn new(meta: &'a [u8]) -> anyhow::Result<Self> {
        let status_line_index = end_of_line(meta);
        let mut status_line = std::str::from_utf8(&meta[..status_line_index])
            .context("status line contains non-UTF8 bytes")?
            .split_ascii_whitespace();
        // TODO: check for HTTP version
        let status_code = status_line
            .nth(1)
            .ok_or_else(|| anyhow::Error::msg("status line has no status code"))?;
        Ok(Self {
            code: status_code
                .parse()
                .context("status code is not a u16 integer")?,
            reason_phrase: status_line.next().unwrap_or_default(),
        })
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.code)
    }
}
