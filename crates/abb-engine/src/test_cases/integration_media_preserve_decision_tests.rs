use std::fs;
use std::path::Path;

use abb_media_pipeline::audio::AudioIntent;
use abb_test_fixtures::media::ffmpeg;

use super::integration_session_desk::Desk;
use crate::session::{AudioEdit, SessionIntent, TitlePlan};

const CHANGED_SINCE_INSPECTION: &str =
    "Invalid input: Audio changed since chapter inspection. Remove and import it again.";

fn opus_source(path: &Path, muxer: &str) {
    ffmpeg([
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=1",
        "-c:a",
        "libopus",
        "-b:a",
        "32k",
        "-f",
        muxer,
        path.to_str().expect("utf8 path"),
    ]);
}

#[tokio::test]
async fn a_source_replaced_after_import_fails_its_plan_before_any_codec_is_read() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/book.m4b"), 1.0)
        .await;
    desk.import(&book).await;
    desk.wait_until("first plan", |d| d.plan() != TitlePlan::Pending)
        .await;

    fs::remove_file(&book).expect("remove source");
    opus_source(&book, "mp4");
    let title_ids = desk.title_ids();
    desk.send(SessionIntent::SetTitleAudio {
        title_ids,
        edit: AudioEdit::Intent(AudioIntent::Preserve),
    })
    .await;
    desk.wait_until("replanned", |d| {
        matches!(d.plan(), TitlePlan::Failed { .. })
    })
    .await;

    let TitlePlan::Failed { message, .. } = desk.plan() else {
        panic!("{:?}", desk.plan());
    };
    assert_eq!(message, CHANGED_SINCE_INSPECTION);
}
