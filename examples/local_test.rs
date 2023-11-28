use anyhow::Context;

const HOST: &str = "http://127.0.0.1:8888/";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let http = http_chunked::http::Context::new(HOST)
        .await
        .context("create http context")?;
    dbg!(http.host_header());
    dbg!(http.host_header().to_string());
    Ok(())
}
