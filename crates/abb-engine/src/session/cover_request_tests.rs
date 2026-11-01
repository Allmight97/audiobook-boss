use super::*;

#[test]
fn a_request_decodes_once_around_the_whole_and_once_around_its_value() {
    let url = "https://covers.test/a b/c.jpg?x=1";
    let value = percent_encoding::utf8_percent_encode(url, percent_encoding::NON_ALPHANUMERIC);
    let whole = format!("remote/small/0/{value}");
    let request = percent_encoding::utf8_percent_encode(&whole, percent_encoding::NON_ALPHANUMERIC);
    assert_eq!(
        CoverRequest::parse(&format!("/{request}")).expect("parses"),
        CoverRequest::Remote {
            url: url.to_string(),
            small: true
        }
    );
}

#[test]
fn only_known_kinds_and_sizes_parse() {
    for request in [
        "audio/full/0/x",
        "session/small/1/",
        "remote/huge/0/x",
        "elsewhere/small/0/x",
        "remote/small",
    ] {
        assert!(CoverRequest::parse(request).is_err(), "{request}");
    }
    assert_eq!(
        CoverRequest::parse("session/full/7/").expect("session cover"),
        CoverRequest::Session { revision: 7 }
    );
}
