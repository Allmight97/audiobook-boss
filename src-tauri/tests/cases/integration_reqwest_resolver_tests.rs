use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

const LOCAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

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
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        let (mut stream, _) = timeout(LOCAL_REQUEST_TIMEOUT, listener.accept()).await??;
        let mut buffer = [0u8; 1024];
        let _ = timeout(LOCAL_REQUEST_TIMEOUT, stream.read(&mut buffer)).await??;
        timeout(
            LOCAL_REQUEST_TIMEOUT,
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"),
        )
        .await??;
        Ok::<(), std::io::Error>(())
    });

    let resolver = Arc::new(PortZeroResolver { ip: addr.ip() });
    let client = reqwest::Client::builder()
        .dns_resolver(resolver)
        .no_proxy()
        .timeout(LOCAL_REQUEST_TIMEOUT)
        .build()?;
    let url = format!("http://example.test:{}/", addr.port());
    let response = timeout(LOCAL_REQUEST_TIMEOUT, client.get(url).send()).await??;
    let body = timeout(LOCAL_REQUEST_TIMEOUT, response.text()).await??;

    assert_eq!(body, "OK");
    server.await??;

    Ok(())
}
