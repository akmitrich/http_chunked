const HOST: &str = "mail.ru:80";
const _KEY: &str = "ABCD67520001";

#[tokio::main]
async fn main() {
    let mut http = http_chunked::HttpContext::new(HOST).await.unwrap();
    http.begin();
    loop {
        http.begin_request(http_chunked::Method::Get, "/mail");
        {
            http.request_header("Host", HOST);
            http.request_header("Foo", "Bar");
            http.request_header("Hello", "World");
            http.request_headers_end();
            http.request_body_begin();
            http.request_body_chunk(b"Hello immediate World!\n");
            http.request_body_chunk(b"Test, test\nTest!\n");
        }
        http.end_request();

        //     http.begin_response();
        //     if http.status().is_success() {
        //         for chunk in http.response_chunk_iter() {}
        //     }
        //     http.response_end();
        break;
    }
    http.end();
    println!("OK.")
}
