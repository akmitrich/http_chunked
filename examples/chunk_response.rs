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
        println!(
            "Before {:?} -> {:?}",
            http.state(),
            std::str::from_utf8(&http.rollin)
        );
        for header in http.response_header_iter() {
            println!("{:?}", header);
        }
        println!("{}", "-".repeat(40));
        let mut buf = [0; 4096];
        for i in 1.. {
            let n = http.response_body_chunk_read(&mut buf).await?;
            println!("Chunk {}. {:?}", i, std::str::from_utf8(&buf[..n]));
            if !http.has_response() {
                break;
            }
        }

        println!(
            "After {:?} -> {:?}",
            http.state(),
            std::str::from_utf8(&http.rollin)
        );
        http.response_end();
    }
    http.end();
    Ok(println!("Ok."))
}
