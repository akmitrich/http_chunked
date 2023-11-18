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
            http.request_headers_end().await?;
            http.request_body_chunk(b"Hello immediate World!\n").await?;
            http.request_body_chunk(b"Test, test\nTest!\n").await?;
        }
        http.end_request();
        println!("The request has been sent.\n{}", "-".repeat(40));

        http.response_begin().await?;
        if http.status()?.is_success() {
            println!(
                "Status: {:?}\n\nHeaders:\n{}",
                http.status()?,
                "-".repeat(40)
            );
            for header in http.response_header_iter() {
                println!("{}", std::str::from_utf8(header).unwrap());
            }
            println!("{}", "-".repeat(40));
            println!("Body length: {:?} bytes", http.response_body_left())
            // println!(
            //     "Body chunk: {:?}\n{}",
            //     http.status(),
            //     std::str::from_utf8(&http.response_body_chunk).context("non-UTF8 in body chunk")?
            // );
        }
        http.response_end();
        break;
    }
    http.end();
    Ok(println!("OK."))
}
