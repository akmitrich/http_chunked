use std::io::Write;

use anyhow::Context;

const HOST: &str = "api.asmsolutions.ru:80";
const _KEY: &str = "ABCD67520001";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut http = http_chunked::HttpContext::new(HOST).await?;
    http.begin();
    loop {
        http.begin_request(http_chunked::Method::Get, "/").await?;
        {
            http.request_header("Host", HOST).await?;
            http.request_header("Foo", "Bar").await?;
            http.request_header("Hello", "World").await?;
            http.request_header("Content-Length", "0").await?;
            http.request_headers_end().await?;
            http.request_body_chunk(b"Hello immediate World!\n").await?;
            http.request_body_chunk(b"Test, test\nTest!\n").await?;
        }
        http.end_request();
        println!("The request has been sent.\n{}", "-".repeat(40));

        http.response_begin().await?;
        println!(
            "Status: {:?}\n\nHeaders:\n{}",
            http.status()?,
            "-".repeat(40)
        );
        let mut bytes_left = 0;
        for header in http.response_header_iter() {
            println!("{:?}", header);
            if let http_chunked::HttpHeader::ContentLength(len) = header {
                bytes_left = len;
            }
        }
        println!("{}", "-".repeat(40));
        if http.status()?.is_success() {
            println!("Body length: {:?} bytes", bytes_left);
            let mut output = std::fs::File::create("../get.pdf").context("create output file")?;
            let mut buf = [0; 1024];
            let mut total = 0;
            while bytes_left > 0 {
                let n = http.response_body_chunk_read(&mut buf).await?;
                output
                    .write_all(&buf[..n])
                    .context("write to output file")?;
                total += n;
                bytes_left -= n;
            }
            println!("{} bytes saved to {:?}", total, output);
        }
        http.response_end();
        break;
    }
    http.end();
    Ok(println!("OK."))
}
