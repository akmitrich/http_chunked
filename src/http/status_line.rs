use anyhow::Context;

use super::end_of_line;

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub struct Status<'a> {
    pub http_version: &'a str,
    pub code: u16,
    pub reason_phrase: &'a str,
}

impl<'a> Status<'a> {
    pub fn new(meta: &'a [u8]) -> anyhow::Result<Self> {
        let status_line_index = end_of_line(meta);
        let mut status_line = std::str::from_utf8(&meta[..status_line_index])
            .context("status line contains non-UTF8 bytes")?
            .split_ascii_whitespace();
        let http_version = status_line
            .next()
            .ok_or_else(|| anyhow::Error::msg("cannot find HTTP-version"))?;
        if !http_version.contains("/1.") {
            return Err(anyhow::Error::msg(
                "unacceptable HTTP version; accept 1.0 or 1.1",
            ));
        }
        let status_code = status_line
            .next()
            .ok_or_else(|| anyhow::Error::msg("status line has no status code"))?;
        Ok(Self {
            http_version,
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
