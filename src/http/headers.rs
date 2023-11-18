use super::{get_line, skip_line};

#[derive(Debug)]
pub struct HeaderIter<'a> {
    cursor: &'a [u8],
    content_length: Option<usize>,
}

impl<'a> Iterator for HeaderIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.is_empty() {
            return None;
        }
        let result = get_line(self.cursor);
        self.cursor = skip_line(self.cursor);
        Some(result)
    }
}

impl<'a> HeaderIter<'a> {
    pub fn new(cursor: &'a [u8]) -> Self {
        Self {
            cursor,
            content_length: None,
        }
    }
}
