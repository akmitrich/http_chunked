use http_chunked::HttpHeader;

const HOST: &str = "http://anglesharp.azurewebsites.net/Chunked";
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut http = http_chunked::http::Context::new(HOST).await?;
    println!("We have http: {:?} (Host={:?})", http.debug(), http.host_header());
    http.begin();
    {
        http.begin_request(http_chunked::Method::Get).await?;
        {
            http.request_header(http.host_header()).await?;
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
        println!("Before {:?} -> {:?}", http.state(), http.debug());
        println!(
            "Status: {:?}\n\nHeaders:\n{}",
            http.status()?,
            "-".repeat(40)
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

        println!("After {:?} -> {:?}", http.state(), http.debug());
        http.response_end();
    }
    http.end();
    Ok(println!("Ok."))
}
