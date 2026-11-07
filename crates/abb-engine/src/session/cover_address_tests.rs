use super::*;

const AWKWARD: &str = "/books/a b/100% (final) ä#1?x=y&z.m4b";

/// What the host sends the engine for an address: the page encodes it once
/// more as a URL path component.
pub(in crate::session) fn host_request(address: &str) -> String {
    format!("/{}", utf8_percent_encode(address, COMPONENT))
}

#[test]
fn every_offered_cover_path_parses_back_to_its_request() {
    let url = "https://covers.test/a b/c.jpg?x=1&y=ä";
    for (address, request) in [
        (
            remote(url),
            CoverRequest::Remote {
                url: url.to_string(),
            },
        ),
        (
            audio(Path::new(AWKWARD), 41),
            CoverRequest::Audio {
                path: PathBuf::from(AWKWARD),
            },
        ),
        (session(7), CoverRequest::Session { revision: 7 }),
        (
            preview("run/1 %"),
            CoverRequest::Preview {
                run_id: "run/1 %".to_string(),
            },
        ),
    ] {
        assert_eq!(
            CoverRequest::parse(&host_request(&address)).expect("parses"),
            request,
            "{address}"
        );
    }
}

#[test]
fn an_address_names_the_revision_it_was_built_for() {
    let path = Path::new(AWKWARD);
    assert_ne!(audio(path, 1), audio(path, 2));
    assert_ne!(session(1), session(2));
    assert_eq!(audio(path, 3), audio(path, 3));
}

#[test]
fn only_known_kinds_parse() {
    for request in ["elsewhere/0/x", "remote/0", "session/x/", "remote"] {
        assert!(CoverRequest::parse(request).is_err(), "{request}");
    }
    assert_eq!(
        CoverRequest::parse("session/7/").expect("session cover"),
        CoverRequest::Session { revision: 7 }
    );
}
