mod context;
pub mod headers;
pub mod status_line;

pub use context::HttpContext;

fn end_of_line(line: &[u8]) -> usize {
    line.windows(2)
        .enumerate()
        .find(|(_, w)| w.eq(b"\r\n"))
        .map(|(i, _)| i)
        .unwrap_or_else(|| line.len())
}

fn get_line(line: &[u8]) -> &[u8] {
    let end_of_line = end_of_line(line);
    &line[..end_of_line]
}

fn skip_line(line: &[u8]) -> &[u8] {
    let end_of_line = end_of_line(line);
    if end_of_line == line.len() {
        &line[end_of_line..end_of_line]
    } else {
        &line[(end_of_line + 2)..]
    }
}
