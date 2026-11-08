use super::*;
use crate::remote_source::test_support::{Reply, StubServer};

const HOST: &str = "prowlarr.test";

fn client(stub: &StubServer) -> ReqwestProwlarrAdapter {
    ReqwestProwlarrAdapter::with_client(
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .resolve(HOST, stub.address())
            .build()
            .expect("client"),
    )
}

fn key() -> SecretString {
    SecretString::from("secret-key".to_string())
}

async fn search_message(status: &str, body: &str) -> String {
    let stub = StubServer::start(vec![Reply::json(status, body)]).await;
    let outcome = client(&stub)
        .search(
            &format!("http://{HOST}"),
            &key(),
            &ProwlarrSearchParams {
                query: "test".to_string(),
                category_ids: vec![3030],
            },
        )
        .await
        .expect("a failed search is an outcome");
    assert_eq!(outcome.diagnostics.len(), 1);
    outcome.diagnostics[0].message.clone()
}

async fn system_status_message(status: &str, body: &str) -> String {
    let stub = StubServer::start(vec![Reply::json(status, body)]).await;
    client(&stub)
        .system_status(&format!("http://{HOST}"), &key())
        .await
        .expect("a failed connection test is an outcome")
        .message
}

#[tokio::test]
async fn a_failed_search_shows_only_the_json_message_without_addresses() {
    let message = search_message(
        "500 Internal Server Error",
        r#"{"message":"Indexer down at http://tracker.example/dl?passkey=SECRET or host.example/dl?passkey=SECRET2","description":"SECRET3"}"#,
    )
    .await;

    assert_eq!(
        message,
        "Indexer search failed with status 500 Internal Server Error: Indexer down at <url> or <url>"
    );
}

#[tokio::test]
async fn a_failed_search_with_a_non_json_body_shows_the_status_only() {
    let message = search_message(
        "502 Bad Gateway",
        "<html>Bad gateway http://tracker.example/?passkey=SECRET</html>",
    )
    .await;

    assert_eq!(message, "Indexer search failed with status 502 Bad Gateway");
}

#[tokio::test]
async fn a_failed_search_message_is_cut_to_200_characters() {
    let body = format!(r#"{{"message":"{}"}}"#, "x".repeat(300));

    let message = search_message("500 Internal Server Error", &body).await;

    assert_eq!(
        message,
        format!(
            "Indexer search failed with status 500 Internal Server Error: {}",
            "x".repeat(200)
        )
    );
}

#[tokio::test]
async fn a_failed_connection_test_shows_only_the_json_message_without_addresses() {
    let message = system_status_message(
        "503 Service Unavailable",
        r#"{"message":"Cannot reach www.tracker.example/dl or host.example/dl?passkey=SECRET"}"#,
    )
    .await;

    assert_eq!(
        message,
        "Indexer connection test failed with status 503 Service Unavailable: Cannot reach <url> or <url>"
    );
}

#[tokio::test]
async fn a_failed_connection_test_with_a_non_json_body_shows_the_status_only() {
    let message = system_status_message(
        "500 Internal Server Error",
        "Server error at http://tracker.example/?passkey=SECRET",
    )
    .await;

    assert_eq!(
        message,
        "Indexer connection test failed with status 500 Internal Server Error"
    );
}

#[test]
fn a_scheme_less_word_with_a_query_value_is_an_address() {
    for word in ["host/dl?passkey=SECRET", "tracker.example?key=v", "a?b=c"] {
        assert!(looks_like_url(word), "{word}");
    }
    for word in ["really?", "what?!", "key=value", "Prowlarr", "ok"] {
        assert!(!looks_like_url(word), "{word}");
    }
}

#[tokio::test]
async fn a_rejected_api_key_is_one_outcome_for_every_request_and_both_statuses() {
    for status in ["401 Unauthorized", "403 Forbidden"] {
        assert_eq!(
            search_message(status, r#"{"message":"raw"}"#).await,
            "Indexer rejected the API key.",
            "{status}"
        );
        assert_eq!(
            system_status_message(status, r#"{"message":"raw"}"#).await,
            "Indexer rejected the API key.",
            "{status}"
        );
    }
}
