use super::tests::{release, serve_one, start_listener};
use super::*;
use std::time::Duration;

#[tokio::test]
async fn an_answer_that_cannot_be_read_shows_a_fixed_line_without_the_address() {
    for (status, unconfirmed, line) in [
        (
            "200 OK",
            true,
            "Prowlarr's answer could not be read, so this release may already be sent. Check your downloader before you Retry.",
        ),
        (
            "500 Internal Server Error",
            false,
            "Prowlarr's answer could not be read. Details are in the log.",
        ),
    ] {
        let (listener, addr) = start_listener().await;
        #[expect(
            clippy::disallowed_methods,
            reason = "joined by an_answer_that_cannot_be_read_shows_a_fixed_line_without_the_address"
        )]
        let server = tokio::spawn(async move {
            let response = format!(
                "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 100\r\n\r\npartial"
            );
            serve_one(&listener, response.as_bytes()).await;
        });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .resolve("prowlarr.test", addr)
            .build()
            .expect("client");

        let outcome = ReqwestProwlarrAdapter::with_client(client)
            .grab(
                &format!("http://prowlarr.test:{}", addr.port()),
                &SecretString::from("secret-key".to_string()),
                &release("Example", RemoteReleaseProtocol::Torrent),
                "test-grab",
            )
            .await
            .expect("an unreadable answer is an outcome");

        server.await.expect("server");
        assert!(!outcome.accepted, "{status}");
        assert_eq!(outcome.unconfirmed, unconfirmed, "{status}");
        assert_eq!(outcome.message, line, "{status}");
    }
}
