use super::*;
use crate::remote_source::providers::audible::http::stub::stub_api;

const BOOK: &str = r#"{"item":{"title":"Book","authors":[{"name":"Ann Author"}],"series":[{"title":"Saga","sequence":"1"}],"release_date":"2021-03-04"}}"#;

struct Looked {
    title: std::result::Result<PdfTitle, RemoteSourceDiagnostic>,
    requests: Vec<String>,
}

async fn look_up(status: u16, body: &str) -> Looked {
    let (api, server) = stub_api(vec![(status, body.to_string())]).await;
    let title = lookup_pdf_title(&api, "job-1", "B000000001", &|| false)
        .await
        .expect("not cancelled");
    Looked {
        title,
        requests: server
            .finished()
            .await
            .iter()
            .map(|request| request.request_line().to_string())
            .collect(),
    }
}

#[tokio::test]
async fn the_lookup_reads_the_facts_that_name_the_pdf_folder() {
    let looked = look_up(200, BOOK).await;

    assert_eq!(looked.requests.len(), 1);
    assert!(looked.requests[0].starts_with("GET /1.0/library/B000000001?"));
    assert!(
        looked.requests[0].contains("series"),
        "{}",
        looked.requests[0]
    );
    assert_eq!(
        looked.title.expect("named title"),
        PdfTitle {
            title: "Book".into(),
            authors: vec!["Ann Author".into()],
            series: Some("Saga".into()),
            series_part: Some("1".into()),
            release_date: Some("2021-03-04".into()),
        }
    );
}

#[tokio::test]
async fn a_rejected_sign_in_on_the_lookup_refuses_the_save() {
    let looked = look_up(401, "{}").await;

    assert_eq!(
        looked.title.expect_err("refused").kind,
        RemoteAcquisitionFailureKind::AuthRequired
    );
}

#[tokio::test]
async fn a_title_audible_does_not_name_is_refused() {
    let looked = look_up(200, r#"{"item":{"authors":[]}}"#).await;

    assert_eq!(looked.title.expect_err("refused").message, NO_TITLE);
}

#[test]
fn a_refused_companion_file_fails_the_save_as_an_unsaved_pdf() {
    let diagnostic = download_failure(
        "B000000001",
        SupplementalPdfFailure {
            category: PdfFailure::Status,
            status: Some(reqwest::StatusCode::NOT_FOUND),
        },
    );

    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::SupplementalPdfFailed
    );
    assert_eq!(diagnostic.title_id.as_deref(), Some("B000000001"));
    assert!(diagnostic
        .message
        .ends_with("The Supplemental PDF was not saved."));
    assert!(
        !diagnostic.message.contains("audiobook"),
        "{}",
        diagnostic.message
    );
}

#[test]
fn the_download_spans_the_whole_pdf_job_bar() {
    assert_eq!(download_progress("B000000001", 0, None).percentage, 0.0);
    assert_eq!(
        download_progress("B000000001", 512, Some(1024)).percentage,
        DOWNLOAD_SHARE / 2.0
    );
    assert_eq!(
        download_progress("B000000001", 1024, Some(1024)).percentage,
        DOWNLOAD_SHARE
    );
}
