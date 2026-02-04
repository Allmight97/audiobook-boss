use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct PortZeroResolver {
    ip: IpAddr,
}

impl Resolve for PortZeroResolver {
    fn resolve(&self, _name: Name) -> Resolving {
        let addr = SocketAddr::new(self.ip, 0);
        Box::pin(async move { Ok(Box::new(std::iter::once(addr)) as Addrs) })
    }
}

#[tokio::test]
async fn reqwest_replaces_port_zero_with_url_port() -> Result<(), Box<dyn std::error::Error>> {
    for var in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        std::env::remove_var(var);
    }

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let mut buffer = [0u8; 1024];
        let _ = stream.read(&mut buffer).await?;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
            .await?;
        Ok::<(), std::io::Error>(())
    });

    let resolver = Arc::new(PortZeroResolver { ip: addr.ip() });
    let client = reqwest::Client::builder().dns_resolver(resolver).build()?;
    let url = format!("http://example.test:{}/", addr.port());
    let body = client.get(url).send().await?.text().await?;

    assert_eq!(body, "OK");
    server.await??;

    Ok(())
}
