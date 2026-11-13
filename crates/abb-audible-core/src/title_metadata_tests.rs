use super::*;
use serde_json::json;

#[test]
fn title_metadata_reads_the_title_and_either_pdf_alias() {
    for key in ["pdf_url", "pdfUrl"] {
        let metadata = title_metadata_from_value(&json!({
            "product": { "title": "Book", key: "https://example.test/book.pdf" }
        }));
        assert_eq!(metadata.title.as_deref(), Some("Book"), "{key}");
        assert_eq!(
            metadata.supplemental_pdf_url.as_deref(),
            Some("https://example.test/book.pdf"),
            "{key}"
        );
    }
}

#[test]
fn a_blank_title_and_an_empty_pdf_url_read_as_absent() {
    let metadata = title_metadata_from_value(&json!({"title": "  ", "pdf_url": ""}));
    assert_eq!(metadata.title, None);
    assert_eq!(metadata.supplemental_pdf_url, None);
}

#[test]
fn naming_facts_come_from_the_book_and_its_first_series() {
    let cases = [
        (
            json!({"item": {
                "title": "Book Two",
                "authors": [{"asin": "A1", "name": "Ann Author"}, {"name": "Bo Writer"}],
                "series": [
                    {"asin": "S1", "title": "Saga", "sequence": "2"},
                    {"asin": "S2", "title": "Other Saga", "sequence": "7"}
                ],
                "release_date": "2021-03-04"
            }}),
            TitleNamingFacts {
                title: Some("Book Two".into()),
                authors: vec!["Ann Author".into(), "Bo Writer".into()],
                series: Some("Saga".into()),
                series_part: Some("2".into()),
                release_date: Some("2021-03-04".into()),
            },
        ),
        (
            json!({"product": {"title": "Alone", "series": []}}),
            TitleNamingFacts {
                title: Some("Alone".into()),
                ..TitleNamingFacts::default()
            },
        ),
        (
            json!({"item": {"series": [{"title": "Saga", "sequence": " "}], "title": " "}}),
            TitleNamingFacts {
                series: Some("Saga".into()),
                ..TitleNamingFacts::default()
            },
        ),
    ];
    for (reply, expected) in cases {
        assert_eq!(title_naming_facts_from_value(&reply), expected, "{reply}");
    }
}
