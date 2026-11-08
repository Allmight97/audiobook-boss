use super::*;
use crate::remote_source::scoped_output::partial_sibling;
use crate::remote_source::test_support::{CapturedRequest, Reply, StubServer};
use std::sync::atomic::{AtomicBool, Ordering};

const BODY_LEN: usize = 1000;

#[derive(Clone, Copy)]
enum Serve {
    Full,
    /// Send this many bytes of the requested range, then close the connection.
    Truncate(usize),
    Pause(usize),
    /// Read the request, then close without a response.
    Close,
    /// Send the requested range in full, claiming a different total size.
    Resized,
}

struct Stub {
    url: String,
    client: reqwest::Client,
    server: StubServer,
}

impl Stub {
    fn range_starts(&self) -> Vec<u64> {
        self.server.requests().iter().map(range_start).collect()
    }

    fn user_agents(&self) -> Vec<String> {
        self.server
            .requests()
            .iter()
            .map(|request| request.header("user-agent").unwrap_or_default())
            .collect()
    }
}

fn body() -> Vec<u8> {
    (0..BODY_LEN).map(|index| (index % 251) as u8).collect()
}

fn range_start(request: &CapturedRequest) -> u64 {
    request
        .header("range")
        .and_then(|range| {
            range
                .strip_prefix("bytes=")?
                .trim_end_matches('-')
                .parse()
                .ok()
        })
        .expect("ranged request")
}

fn ranged_reply(serve: Serve, start: u64) -> Reply {
    if matches!(serve, Serve::Close) {
        return Reply::close_silently();
    }
    let body = body();
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
    let with_head = |bytes: &[u8]| [head.as_bytes(), bytes].concat();
    match serve {
        Serve::Close => Reply::close_silently(),
        Serve::Full | Serve::Resized => Reply::bytes(with_head(remaining)),
        Serve::Truncate(sent) => Reply::bytes(with_head(&remaining[..sent.min(remaining.len())])),
        Serve::Pause(sent) => Reply::chunks(
            vec![with_head(&remaining[..sent]), remaining[sent..].to_vec()],
            std::time::Duration::from_millis(100),
        ),
    }
}

async fn start_stub(script: Vec<Serve>) -> Stub {
    let server = StubServer::start_tls(move |index, request| match script.get(index) {
        Some(serve) => ranged_reply(*serve, range_start(request)),
        None => Reply::hold(),
    })
    .await;
    Stub {
        url: format!("{}/book.aaxc", server.url()),
        client: server.tls_client(),
        server,
    }
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
    assert_eq!(stub.range_starts(), [0, 400]);
    assert_eq!(stub.user_agents(), [AUDIBLE_DOWNLOAD_USER_AGENT; 2]);
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
    assert_eq!(stub.range_starts(), [0, 400, 400]);
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
    assert_eq!(stub.range_starts(), [0, 100, 100, 100, 100]);
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

#[test]
fn retry_reasons_log_and_report_under_their_old_names() {
    for (reason, logged) in [
        (RetryReason::Request, "request"),
        (RetryReason::Read, "read"),
        (RetryReason::Incomplete, "incomplete"),
    ] {
        assert_eq!(reason.to_string(), logged);
        assert_eq!(
            download_failure(reason).to_string(),
            format!(
                "Operation failed: Audible download {logged} failed. Provider-private details were withheld from UI and logs."
            )
        );
    }
}
