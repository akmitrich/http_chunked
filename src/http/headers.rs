use super::end_of_line;

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
        let end_index = end_of_line(self.cursor);
        let result = &self.cursor[..end_index];
        self.cursor = if end_index == self.cursor.len() {
            &self.cursor[end_index..end_index]
        } else {
            &self.cursor[(end_index + 2)..]
        };
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
