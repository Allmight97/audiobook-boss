use super::tests::release;
use super::*;
use crate::remote_source::test_support::{Reply, StubServer};
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
        let stub = StubServer::start(vec![Reply::bytes(format!(
            "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 100\r\n\r\npartial"
        ))])
        .await;
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .resolve("prowlarr.test", stub.address())
            .build()
            .expect("client");

        let outcome = ReqwestProwlarrAdapter::with_client(client)
            .grab(
                &format!("http://prowlarr.test:{}", stub.address().port()),
                &SecretString::from("secret-key".to_string()),
                &release("Example", RemoteReleaseProtocol::Torrent),
                "test-grab",
            )
            .await
            .expect("an unreadable answer is an outcome");

        stub.finished().await;
        assert!(!outcome.accepted, "{status}");
        assert_eq!(outcome.unconfirmed, unconfirmed, "{status}");
        assert_eq!(outcome.message, line, "{status}");
    }
}

#[tokio::test]
async fn a_search_reply_of_the_wrong_shape_never_quotes_its_body_in_the_error() {
    let body = r#"[{"guid":"abc","size":"https://tracker.test/dl?passkey=SECRET123"}]"#;
    let stub = StubServer::start(vec![Reply::json("200 OK", body)]).await;
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .resolve("prowlarr.test", stub.address())
        .build()
        .expect("client");

    let error = ReqwestProwlarrAdapter::with_client(client)
        .search(
            &format!("http://prowlarr.test:{}", stub.address().port()),
            &SecretString::from("secret-key".to_string()),
            &ProwlarrSearchParams {
                query: "book".to_string(),
                category_ids: vec![3030],
            },
        )
        .await
        .expect_err("a reply of the wrong shape is an error");

    let shown = error.to_string();
    assert!(!shown.contains("SECRET123"), "{shown}");
    assert!(!shown.contains("tracker.test"), "{shown}");
}
