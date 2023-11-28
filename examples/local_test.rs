use anyhow::Context;

const HOST: &str = "127.0.0.1:8888";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut http = http_chunked::http::Context::new(HOST)
        .await
        .context("create http context")?;
    http.begin();
    http.end();
    Ok(())
}
