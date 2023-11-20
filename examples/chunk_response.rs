use anyhow::Context;
use http_chunked::HttpHeader;
use std::io::Write;

const HOST: &str = "anglesharp.azurewebsites.net:80";
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut http = http_chunked::HttpContext::new(HOST).await?;
    http.begin();
    {
        http.begin_request(http_chunked::Method::Get, "/Chunked")
            .await?;
        {
            http.request_header(HttpHeader::from_name_value("Host", HOST)?)
                .await?;
            http.request_header(HttpHeader::from_name_value("Foo", "Bar")?)
                .await?;
            http.request_header(HttpHeader::from_name_value("Hello", "World")?)
                .await?;
            http.request_header(HttpHeader::ContentLength(0)).await?;
            http.request_headers_end().await?;
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
    }
    http.end();
    Ok(println!("Ok."))
}
