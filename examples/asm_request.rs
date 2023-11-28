use anyhow::Context;
use http_chunked::HttpHeader;
use std::io::Write;

const HOST: &str = "api.asmsolutions.ru:80";
const _KEY: &str = "ABCD67520001";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut http = http_chunked::http::Context::new(HOST).await?;
    http.begin();
    {
        http.begin_request(http_chunked::Method::Get, "/").await?;
        {
            http.request_header(HttpHeader::from_name_value("Host", HOST)?)
                .await?;
            http.request_header(HttpHeader::from_name_value("Foo", "Bar")?)
                .await?;
            http.request_header(HttpHeader::from_name_value("Hello", "World")?)
                .await?;
            http.request_header(HttpHeader::ContentLength(0)).await?;
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
        for header in http.response_header_iter() {
            println!("{:?}", header);
        }
        println!("{}", "-".repeat(40));
        if http.status()?.is_success() {
            println!("Body length: {:?} bytes", http.content_length());
            let mut output = std::fs::File::create("../get.pdf").context("create output file")?;
            let mut buf = [0; 4096];
            let mut total = 0;
            while http.has_response() {
                let n = http.response_body_chunk_read(&mut buf).await?;
                println!("n = {}, total = {}", n, total);
                if n == 0 {
                    break;
                }
                output
                    .write_all(&buf[..n])
                    .context("write to output file")?;
                total += n;
            }
            println!("Has written {} bytes", total);
        }
        http.response_end();
    }
    http.end();
    Ok(println!("OK."))
}
