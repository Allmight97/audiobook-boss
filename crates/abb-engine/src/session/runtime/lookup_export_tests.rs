use super::*;
use crate::test_cases::media_fixtures::{picture_codecs, MediaLane};
use crate::work_runtime::{OperationPhase, WorkOperationStatus};
use abb_media_pipeline::audio::EncoderType;
use abb_test_fixtures::poll::{until, until_true};

fn looked_up() -> OnlineMetadataResult {
    OnlineMetadataResult {
        authors: vec!["Looked Author".to_string()],
        narrators: vec!["Looked Narrator".to_string()],
        series: Some("Looked Saga".to_string()),
        series_part: Some("4".to_string()),
        description: Some("Looked blurb".to_string()),
        ..result("Looked Title")
    }
}

#[tokio::test]
async fn a_looked_up_result_and_its_cover_reach_the_exported_file() {
    let rig = rig();
    let lane = MediaLane::with_fixtures(&[1.0]);
    let source = lane
        .export(MetadataIntentPatch {
            title: Some(PatchOp::Set("Alpha".to_string())),
            artist: Some(PatchOp::Set("Source Author".to_string())),
            genre: Some(PatchOp::Set("Fantasy".to_string())),
            ..Default::default()
        })
        .await;
    rig.send(SessionIntent::Import {
        paths: vec![source.to_string_lossy().into_owned()],
    })
    .await;
    rig.wait_for_titles(1).await;
    rig.select(&[0]).await;
    until_true("the source tags show", || rig.title_shown() == "Alpha").await;

    rig.answer_next_search(Ok(MetadataLookupResponse {
        results: vec![looked_up()],
        diagnostics: Vec::new(),
    }));
    rig.send(SessionIntent::LookupOpen).await;
    rig.send(SessionIntent::LookupSetReplaceCover { replace: true })
        .await;
    rig.send(rig.lookup_apply(0)).await;
    assert_eq!(rig.title_shown(), "Looked Title");

    let exports = lane.tmp.path().join("exports");
    std::fs::create_dir_all(&exports).expect("export folder");
    rig.send(SessionIntent::SetOutputDirectory {
        directory: exports.to_string_lossy().into_owned(),
    })
    .await;
    for edit in [
        AudioEdit::Format(AudiobookFormat::M4b),
        AudioEdit::Intent(AudioIntent::Encode),
        AudioEdit::Encoder(EncoderType::NativeAac),
    ] {
        rig.send(SessionIntent::SetTitleAudio {
            title_ids: vec![rig.title_id(0)],
            edit,
        })
        .await;
    }
    rig.send(SessionIntent::Submit).await;
    let export = until("the export finishes", || {
        rig.session
            .inner
            .deps
            .work
            .list_operations()
            .operations
            .into_iter()
            .find(|operation| operation.phase == OperationPhase::Finished)
    })
    .await;
    assert_eq!(export.status, WorkOperationStatus::Completed);
    let output = PathBuf::from(
        export.children[0]
            .output_path
            .clone()
            .expect("the title published"),
    );

    let read = abb_media_pipeline::metadata::read_metadata(&output).expect("read tags back");
    assert_eq!(read.title.as_deref(), Some("Looked Title"));
    assert_eq!(read.album.as_deref(), Some("Looked Title"));
    assert_eq!(read.artist.as_deref(), Some("Looked Author"));
    assert_eq!(read.composer.as_deref(), Some("Looked Narrator"));
    assert_eq!(read.series.as_deref(), Some("Looked Saga"));
    assert_eq!(read.series_part.as_deref(), Some("4"));
    assert_eq!(read.description.as_deref(), Some("Looked blurb"));
    assert_eq!(read.genre.as_deref(), Some("Fantasy"));
    assert_eq!(read.cover_art, Some(written_cover()));
    assert_eq!(picture_codecs(&output), vec![abb_ffmpeg::CodecId::MJPEG]);
}
