//! Download lifecycle harness: a local TLS stub stands in for Audible's CDN, so
//! resume, retry exhaustion, and cancellation are proven without an account.
use super::*;
use crate::remote_source::scoped_output::partial_sibling;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const BODY_LEN: usize = 1000;

#[derive(Clone, Copy)]
enum Serve {
    /// Send the requested range in full.
    Full,
    /// Send this many bytes of the requested range, then close the connection.
    Truncate(usize),
    /// Send this many bytes, pause, then send the rest.
    Pause(usize),
    /// Read the request, then close without a response.
    Close,
    /// Send the requested range in full, claiming a different total size.
    Resized,
}

struct Stub {
    url: String,
    client: reqwest::Client,
    range_starts: Arc<Mutex<Vec<u64>>>,
}

fn body() -> Vec<u8> {
    (0..BODY_LEN).map(|index| (index % 251) as u8).collect()
}

#[expect(clippy::disallowed_methods, reason = "joined by start_stub")]
async fn start_stub(script: Vec<Serve>) -> Stub {
    let ca_key = rcgen::KeyPair::generate().expect("ca key");
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("ca params");
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_cert = ca_params.self_signed(&ca_key).expect("ca cert");
    let issuer = rcgen::Issuer::new(ca_params, ca_key);
    let leaf_key = rcgen::KeyPair::generate().expect("leaf key");
    let leaf_cert = rcgen::CertificateParams::new(vec!["127.0.0.1".to_string()])
        .expect("leaf params")
        .signed_by(&leaf_key, &issuer)
        .expect("leaf cert");

    let server_config = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![leaf_cert.der().clone()],
            tokio_rustls::rustls::pki_types::PrivateKeyDer::Pkcs8(leaf_key.serialize_der().into()),
        )
        .expect("server config");
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub");
    let address = listener.local_addr().expect("stub address");
    let range_starts = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&range_starts);

    tokio::spawn(async move {
        let body = body();
        for serve in script {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            let Ok(mut tls) = acceptor.accept(tcp).await else {
                return;
            };
            let start = read_range_start(&mut tls).await;
            recorded.lock().expect("ranges").push(start);
            if matches!(serve, Serve::Close) {
                let _ = tls.shutdown().await;
                continue;
            }
            let remaining = &body[start as usize..];
            let total = if matches!(serve, Serve::Resized) {
                BODY_LEN + 1
            } else {
                BODY_LEN
            };
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                remaining.len(),
                start,
                BODY_LEN - 1,
                total
            );
            let _ = tls.write_all(head.as_bytes()).await;
            match serve {
                Serve::Close => {}
                Serve::Full | Serve::Resized => {
                    let _ = tls.write_all(remaining).await;
                }
                Serve::Truncate(sent) => {
                    let _ = tls.write_all(&remaining[..sent.min(remaining.len())]).await;
                }
                Serve::Pause(sent) => {
                    let _ = tls.write_all(&remaining[..sent]).await;
                    let _ = tls.flush().await;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    let _ = tls.write_all(&remaining[sent..]).await;
                }
            }
            let _ = tls.flush().await;
            let _ = tls.shutdown().await;
        }
    });

    let client = reqwest::Client::builder()
        .tls_certs_only([reqwest::Certificate::from_der(ca_cert.der()).expect("ca")])
        .build()
        .expect("client");
    Stub {
        url: format!("https://{address}/book.aaxc"),
        client,
        range_starts,
    }
}

async fn read_range_start(tls: &mut tokio_rustls::server::TlsStream<tokio::net::TcpStream>) -> u64 {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = tls.read(&mut buffer).await.expect("read request");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8_lossy(&request)
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("range: bytes=")
                .and_then(|range| range.trim_end_matches('-').parse().ok())
        })
        .expect("ranged request")
}

const LOG: DownloadLogContext<'static> = DownloadLogContext {
    job_id: "job-harness",
    title_id: "B000000001",
    extension: "aaxc",
};

#[tokio::test]
async fn a_truncated_download_resumes_to_the_exact_bytes() {
    let stub = start_stub(vec![Serve::Truncate(400), Serve::Full]).await;
    let root = tempfile::TempDir::new().expect("temp root");
    let target = root.path().join("book.aaxc");

    let bytes = download_to_path_with_client(
        &stub.client,
        &stub.url,
        &target,
        Duration::ZERO,
        LOG,
        &mut |_| {},
        &|| false,
    )
    .await
    .expect("resumed download");

    assert_eq!(bytes, BODY_LEN as u64);
    assert_eq!(std::fs::read(&target).expect("downloaded bytes"), body());
    assert_eq!(*stub.range_starts.lock().expect("ranges"), [0, 400]);
    assert!(!partial_sibling(&target).exists());
}

#[tokio::test]
async fn a_refused_reconnect_is_retried_and_keeps_the_bytes() {
    let stub = start_stub(vec![Serve::Truncate(400), Serve::Close, Serve::Full]).await;
    let root = tempfile::TempDir::new().expect("temp root");
    let target = root.path().join("book.aaxc");

    let bytes = download_to_path_with_client(
        &stub.client,
        &stub.url,
        &target,
        Duration::ZERO,
        LOG,
        &mut |_| {},
        &|| false,
    )
    .await
    .expect("resumed download");

    assert_eq!(bytes, BODY_LEN as u64);
    assert_eq!(std::fs::read(&target).expect("downloaded bytes"), body());
    assert_eq!(*stub.range_starts.lock().expect("ranges"), [0, 400, 400]);
}

#[tokio::test]
async fn bytes_that_arrive_reset_the_try_budget() {
    let mut script = vec![Serve::Truncate(100); MAX_DOWNLOAD_ATTEMPTS as usize + 1];
    script.push(Serve::Full);
    let stub = start_stub(script).await;
    let root = tempfile::TempDir::new().expect("temp root");
    let target = root.path().join("book.aaxc");

    let bytes = download_to_path_with_client(
        &stub.client,
        &stub.url,
        &target,
        Duration::ZERO,
        LOG,
        &mut |_| {},
        &|| false,
    )
    .await
    .expect("every try made progress");

    assert_eq!(bytes, BODY_LEN as u64);
    assert_eq!(std::fs::read(&target).expect("downloaded bytes"), body());
}

#[tokio::test]
async fn a_resumed_reply_with_another_total_size_is_rejected() {
    let stub = start_stub(vec![Serve::Truncate(400), Serve::Resized]).await;
    let root = tempfile::TempDir::new().expect("temp root");
    let target = root.path().join("book.aaxc");

    let error = download_to_path_with_client(
        &stub.client,
        &stub.url,
        &target,
        Duration::ZERO,
        LOG,
        &mut |_| {},
        &|| false,
    )
    .await
    .expect_err("the file changed size between tries");

    assert!(
        error.to_string().contains("size"),
        "unexpected error: {error}"
    );
    assert!(!target.exists());
    assert!(!partial_sibling(&target).exists());
}

#[tokio::test]
async fn retry_exhaustion_fails_without_leaving_files() {
    let mut script = vec![Serve::Truncate(100)];
    script.extend([Serve::Close; MAX_DOWNLOAD_ATTEMPTS as usize - 1]);
    let stub = start_stub(script).await;
    let root = tempfile::TempDir::new().expect("temp root");
    let target = root.path().join("book.aaxc");

    let error = download_to_path_with_client(
        &stub.client,
        &stub.url,
        &target,
        Duration::ZERO,
        LOG,
        &mut |_| {},
        &|| false,
    )
    .await
    .expect_err("no try after the first brings bytes");

    assert!(
        error.to_string().contains("request"),
        "unexpected error: {error}"
    );
    assert_eq!(
        *stub.range_starts.lock().expect("ranges"),
        [0, 100, 100, 100, 100]
    );
    assert!(!target.exists());
    assert!(!partial_sibling(&target).exists());
}

#[tokio::test]
async fn cancelling_mid_download_removes_the_partial_file() {
    let stub = start_stub(vec![Serve::Pause(300)]).await;
    let root = tempfile::TempDir::new().expect("temp root");
    let target = root.path().join("book.aaxc");
    let cancelled = AtomicBool::new(false);

    let error = download_to_path_with_client(
        &stub.client,
        &stub.url,
        &target,
        Duration::ZERO,
        LOG,
        &mut |_| cancelled.store(true, Ordering::SeqCst),
        &|| cancelled.load(Ordering::SeqCst),
    )
    .await
    .expect_err("cancelled download");

    assert!(matches!(error, AppError::Cancellation(_)), "{error}");
    assert!(!target.exists());
    assert!(!partial_sibling(&target).exists());
}

#[tokio::test]
async fn cleartext_urls_are_rejected_without_fetching() {
    let root = tempfile::TempDir::new().expect("temp root");
    let target = root.path().join("book.m4b");

    let error = download_to_path(
        "http://provider.example/book.m4b?token=fake-secret",
        &target,
        LOG,
        &mut |_| {},
        &|| false,
    )
    .await
    .expect_err("cleartext URL rejected");

    assert!(error.to_string().contains("must use https"));
    assert!(!error.to_string().contains("fake-secret"));
    assert!(!target.exists());
    assert!(!partial_sibling(&target).exists());
}
