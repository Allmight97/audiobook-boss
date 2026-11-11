use super::*;
use crate::remote_source::test_support::{Reply, StubServer};
use audible_api::auth::localization;
use audible_api::auth::register::Registration;
use serde_json::json;
use std::collections::HashMap;

pub(in crate::remote_source::providers::audible) fn signing_auth(country_code: &str) -> Auth {
    Auth {
        locale: localization::find_by_country_code(country_code).expect("locale"),
        device_registration: Registration {
            device_serial: "device-serial".to_string(),
            client_id: "client-id".to_string(),
            adp_token: "adp-token".to_string(),
            device_private_key: super::super::signing::tests::test_device_key_pem(),
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            expires: 0,
            website_cookies: HashMap::new(),
            store_authentication_cookie: String::new(),
            device_info: json!({}),
            customer_info: json!({ "user_id": "account-1" }),
        },
        authorization_code: "authorization-code".to_string(),
        code_verifier: "code-verifier".to_string(),
    }
}

pub(in crate::remote_source::providers::audible) async fn stub_api(
    replies: Vec<(u16, String)>,
) -> (AudibleApi, StubServer) {
    stub_http(
        replies
            .into_iter()
            .map(|(status, body)| Reply::json(&format!("{status} Stub"), &body))
            .collect(),
    )
    .await
}

async fn stub_http(replies: Vec<Reply>) -> (AudibleApi, StubServer) {
    let server = StubServer::start(replies).await;
    let api = AudibleApi::at(&signing_auth("us"), server.url()).expect("Audible client");
    (api, server)
}

fn library_page(first_id: usize, count: usize) -> String {
    json!({
        "items": (first_id..first_id + count)
            .map(|id| json!({ "asin": format!("B{id:09}"), "title": format!("Book {id}") }))
            .collect::<Vec<_>>()
    })
    .to_string()
}

#[test]
fn requests_go_to_the_signed_in_marketplace() {
    let api = AudibleApi::new(&signing_auth("uk")).expect("Audible client");

    assert_eq!(api.base_url, "https://api.audible.co.uk");
}

#[tokio::test]
async fn a_refused_sign_in_with_a_json_body_is_not_an_empty_library() {
    let (api, server) = stub_api(vec![(
        401,
        r#"{"message":"Unauthorized","items":[]}"#.to_string(),
    )])
    .await;

    let result = super::super::load_all_library_titles(&api).await;
    server.finished().await;

    assert!(
        matches!(result, Err(AudibleRequestError::AuthRejected)),
        "{result:?}"
    );
}

#[tokio::test]
async fn a_truncated_auth_reply_rejects_sign_in_just_like_a_complete_reply() {
    for status in [401, 403] {
        for length in [2, 100] {
            let (api, server) = stub_http(vec![Reply::bytes(format!(
                "HTTP/1.1 {status} Stub\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{{}}"
            ))])
            .await;

            let result = super::super::load_all_library_titles(&api).await;
            server.finished().await;

            assert!(
                matches!(result, Err(AudibleRequestError::AuthRejected)),
                "status={status} content_length={length}: {result:?}"
            );
        }
    }
}

#[tokio::test]
async fn a_failed_second_page_loads_no_partial_library() {
    let (api, server) = stub_api(vec![
        (
            200,
            library_page(0, super::super::AUDIBLE_LIBRARY_PAGE_SIZE as usize),
        ),
        (500, r#"{"items":[]}"#.to_string()),
    ])
    .await;

    let result = super::super::load_all_library_titles(&api).await;
    let requests = server.finished().await;

    let Err(AudibleRequestError::Failed(error)) = result else {
        panic!("a failed page must fail the scan: {result:?}");
    };
    assert!(
        error.to_string().contains("No partial library was loaded"),
        "{error}"
    );
    assert!(
        requests[0].request_line().starts_with("GET /1.0/library?"),
        "{requests:?}"
    );
    assert!(
        requests[1].request_line().contains("page=1"),
        "{requests:?}"
    );
}

fn query_of(request_line: &str) -> HashMap<String, String> {
    let target = request_line.split_whitespace().nth(1).expect("target");
    reqwest::Url::parse(&format!("http://stub{target}"))
        .expect("url")
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

#[tokio::test]
async fn a_library_request_asks_for_availability_and_pdf_fields() {
    let (api, server) = stub_api(vec![(200, library_page(0, 3))]).await;

    let titles = super::super::load_all_library_titles(&api)
        .await
        .expect("library");
    let requests = server.finished().await;

    assert_eq!(titles.len(), 3);
    assert_eq!(requests.len(), 1, "a short first page ends the scan");
    let query = query_of(requests[0].request_line());
    assert_eq!(
        query["num_results"],
        super::super::AUDIBLE_LIBRARY_PAGE_SIZE.to_string()
    );
    assert_eq!(query["sort_by"], "-PurchaseDate");
    assert!(!query.contains_key("page"));
    for group in ["is_playable", "is_visible", "is_downloaded", "pdf_url"] {
        assert!(
            query["response_groups"].split(',').any(|got| got == group),
            "library request must ask Audible for {group}"
        );
    }
}

#[tokio::test]
async fn a_paged_library_loads_every_title_once_and_stops() {
    const PAGE: usize = super::super::AUDIBLE_LIBRARY_PAGE_SIZE as usize;
    let cases = [
        (
            "overlapping pages",
            vec![
                library_page(0, PAGE),
                library_page(500, PAGE),
                library_page(1500, 10),
            ],
            1510,
        ),
        (
            "explicit page 1 repeating the default page",
            vec![
                library_page(0, PAGE),
                library_page(0, PAGE),
                library_page(PAGE, 5),
            ],
            PAGE + 5,
        ),
        (
            "a full page with nothing new",
            vec![
                library_page(0, PAGE),
                library_page(PAGE, PAGE),
                library_page(PAGE, PAGE),
            ],
            2 * PAGE,
        ),
    ];

    for (name, pages, expected_titles) in cases {
        let (api, server) = stub_api(pages.into_iter().map(|page| (200, page)).collect()).await;

        let titles = super::super::load_all_library_titles(&api)
            .await
            .expect("library");
        let requests = server.finished().await;

        let ids = titles
            .iter()
            .map(|title| title.title_id.as_str())
            .collect::<Vec<_>>();
        let unique = ids.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), ids.len(), "{name}: a title loaded twice");
        assert_eq!(ids.len(), expected_titles, "{name}");
        let pages_asked = requests
            .iter()
            .map(|request| query_of(request.request_line()).get("page").cloned())
            .collect::<Vec<_>>();
        assert_eq!(
            pages_asked,
            [None, Some("1".to_string()), Some("2".to_string())],
            "{name}"
        );
    }
}

#[tokio::test]
async fn a_signed_request_signs_the_path_query_and_body_it_sends() {
    let (api, server) = stub_api(vec![(200, "{}".to_string()), (200, library_page(0, 1))]).await;

    api.post(
        "/1.0/content/B0TEST/licenserequest",
        &json!({ "quality": "High" }),
        ApiCall::account("license request"),
    )
    .await
    .expect("license reply");
    super::super::load_all_library_titles(&api)
        .await
        .expect("library");
    let requests = server.finished().await;

    let signer = super::super::signing::AdpSigner::new(
        "adp-token",
        &super::super::signing::tests::test_device_key_pem(),
    )
    .expect("signer");
    for request in &requests {
        let sent = request.header("x-adp-signature").expect("signature header");
        let (_, date) = sent.split_once(':').expect("dated signature");
        let expected = signer
            .headers(
                &request.method,
                &request.path_and_query,
                request.body.as_bytes(),
                date,
            )
            .expect("signed");
        assert_eq!(
            expected["x-adp-signature"],
            sent.as_str(),
            "{}",
            request.request_line()
        );
        assert_eq!(request.header("x-adp-token").as_deref(), Some("adp-token"));
        assert_eq!(
            request.header("x-adp-alg").as_deref(),
            Some("SHA256withRSA:1.0")
        );
        assert_eq!(
            request.header("accept").as_deref(),
            Some("application/json")
        );
        assert_eq!(request.header("accept-charset").as_deref(), Some("utf-8"));
        assert_eq!(
            request.header("content-type").as_deref(),
            Some("application/json")
        );
    }
    assert!(requests[0].body.contains("High"), "{:?}", requests[0]);
    assert!(
        requests[1].path_and_query.contains('?'),
        "{:?}",
        requests[1]
    );
}
