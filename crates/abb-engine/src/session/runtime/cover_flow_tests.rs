use super::*;

#[tokio::test]
async fn a_cover_url_is_required_before_anything_loads() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;

    let outcome = rig
        .send(SessionIntent::LoadCoverFromUrl {
            url: "   ".to_string(),
        })
        .await;

    assert_eq!(outcome, SessionOutcome::CoverLoadFailed);
    let cover = rig.metadata().cover;
    assert_eq!(
        cover.notice,
        Some(crate::session::state::CoverNotice::UrlRequired)
    );
    assert!(!cover.loading);
}

#[tokio::test]
async fn a_cover_loaded_from_a_url_is_staged_on_the_selected_title() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;

    let outcome = rig
        .send(SessionIntent::LoadCoverFromUrl {
            url: " https://example.com/cover.jpg ".to_string(),
        })
        .await;

    assert_eq!(outcome, SessionOutcome::Applied);
    let cover = rig.metadata().cover;
    assert!(cover.image.is_some() && cover.custom && !cover.loading);
    assert_eq!(
        cover.notice,
        Some(crate::session::state::CoverNotice::LoadedFromUrl)
    );
    assert_eq!(rig.cover_shown().await, Some(written_cover()));
}

#[tokio::test]
async fn a_cover_cleared_while_loading_is_not_replaced_by_the_load() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;

    let load = rig.session.begin(SessionIntent::LoadCoverFromUrl {
        url: "https://example.com/cover.jpg".to_string(),
    });
    rig.send(SessionIntent::ClearCover).await;

    assert_eq!(load.finish().await.outcome, SessionOutcome::Superseded);
    assert_eq!(rig.pending("alpha").and_then(|patch| patch.cover_art), None);
    let cover = rig.metadata().cover;
    assert!(!cover.loading && cover.image.is_none());
}

#[tokio::test]
async fn a_cover_loaded_for_a_form_that_was_replaced_reports_the_lost_edit() {
    let (release, pending) = tokio::sync::oneshot::channel();
    let rig = rig_with_cover(Some(pending));
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;

    let load = rig.session.begin(SessionIntent::LoadCoverFromUrl {
        url: "https://example.com/cover.jpg".to_string(),
    });
    rig.select(&[1]).await;
    release.send(Ok(downloaded_cover())).expect("release cover");

    assert_eq!(load.finish().await.outcome, SessionOutcome::EditLost);
    assert_eq!(rig.pending("alpha").and_then(|patch| patch.cover_art), None);
    assert_eq!(rig.pending("beta").and_then(|patch| patch.cover_art), None);
}

#[tokio::test]
async fn a_cover_overtaken_by_a_newer_choice_on_the_same_form_is_superseded() {
    let (release, pending) = tokio::sync::oneshot::channel();
    let rig = rig_with_cover(Some(pending));
    rig.load(&["alpha"]);
    rig.select(&[0]).await;

    let first = rig.session.begin(SessionIntent::LoadCoverFromUrl {
        url: "https://example.com/first.jpg".to_string(),
    });
    let second = rig
        .send(SessionIntent::LoadCoverFromUrl {
            url: "https://example.com/second.jpg".to_string(),
        })
        .await;
    release.send(Ok(downloaded_cover())).expect("release cover");

    assert_eq!(second, SessionOutcome::Applied);
    assert_eq!(first.finish().await.outcome, SessionOutcome::Superseded);
}

#[tokio::test]
async fn covers_are_served_only_for_what_the_session_shows() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let listed = staged_wav(folder.path(), "listed");
    let unlisted = staged_wav(folder.path(), "unlisted");
    rig.send(SessionIntent::Import {
        paths: vec![listed.to_string_lossy().into_owned()],
    })
    .await;
    let request = |path: &std::path::Path| {
        let canonical = path.canonicalize().expect("canonical");
        host_request(&cover_address::audio(&canonical, 0))
    };

    assert_eq!(
        rig.session.cover(&request(&listed)).await.expect("listed"),
        None,
        "a listed source with no cover has none"
    );
    assert!(rig.session.cover(&request(&unlisted)).await.is_err());
    assert!(rig
        .session
        .cover(&host_request(&cover_address::remote(
            "https://covers.test/never-shown.jpg"
        )))
        .await
        .is_err());
    assert_eq!(
        rig.session
            .cover(&host_request(&cover_address::session(0)))
            .await
            .expect("session"),
        None
    );
}

#[tokio::test]
async fn an_unsupported_cover_drop_publishes_the_ingestion_diagnostic() {
    let rig = rig();
    rig.load(&["alpha"]);
    let file = rig._config.path().join("cover.gif");
    std::fs::write(&file, b"GIF89a").expect("unsupported image fixture");
    assert_eq!(
        rig.send(SessionIntent::LoadCoverFromDrop {
            paths: vec![file.to_string_lossy().into_owned()]
        })
        .await,
        SessionOutcome::CoverLoadFailed
    );
    assert!(matches!(
        rig.session
            .snapshot()
            .metadata
            .expect("metadata")
            .cover
            .notice,
        Some(crate::session::CoverNotice::LoadFailed { .. })
    ));
}

#[tokio::test]
async fn text_that_is_not_a_web_address_is_refused_without_a_fetch() {
    for text in [
        "example.com/cover.jpg",
        "note: see cover",
        "ftp://example.com/a.jpg",
    ] {
        let rig = rig();
        rig.load(&["alpha"]);
        rig.select(&[0]).await;

        let outcome = rig
            .send(SessionIntent::LoadCoverFromUrl {
                url: text.to_string(),
            })
            .await;

        assert_eq!(outcome, SessionOutcome::CoverLoadFailed, "{text}");
        let cover = rig.metadata().cover;
        assert_eq!(
            cover.notice,
            Some(crate::session::state::CoverNotice::NotAnImageUrl),
            "{text}"
        );
        assert!(!cover.loading && cover.image.is_none(), "{text}");
    }
}

#[tokio::test]
async fn a_cover_with_no_single_title_to_own_it_reports_failure_for_url_and_file() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0, 1]).await;
    let file = rig._config.path().join("cover.png");
    std::fs::write(&file, downloaded_cover()).expect("cover file");

    let from_url = rig
        .send(SessionIntent::LoadCoverFromUrl {
            url: "https://example.com/cover.jpg".to_string(),
        })
        .await;
    let url_cover = rig.metadata().cover;
    let from_file = rig
        .send(SessionIntent::LoadCoverFromFile {
            path: file.to_string_lossy().into_owned(),
        })
        .await;

    assert_eq!(from_url, SessionOutcome::CoverLoadFailed);
    assert_eq!(from_file, SessionOutcome::CoverLoadFailed);
    assert_eq!(
        url_cover.notice,
        Some(crate::session::state::CoverNotice::NoTitleForCover)
    );
    let cover = rig.metadata().cover;
    assert_eq!(
        cover.notice,
        Some(crate::session::state::CoverNotice::NoTitleForCover)
    );
    assert!(!cover.custom);
    assert_eq!(rig.pending("alpha").and_then(|patch| patch.cover_art), None);
}
