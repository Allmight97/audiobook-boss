use std::fs;
use std::path::PathBuf;

use abb_engine::audio::AudiobookFormat;
use abb_engine::session::{AudioEdit, MetadataField, SessionIntent, SessionOutcome};
use abb_engine::work_runtime::WorkOperationStatus;

use super::integration_session_desk::Desk;
use super::media_fixtures::{audio_duration_ts, chapters_of, ffprobe, mux_chapters, SAMPLE_RATE};

#[tokio::test]
async fn a_preview_plays_ninety_seconds_with_the_edited_metadata_and_no_chapters() {
    let desk = Desk::new();
    let built = desk
        .audiobook(&desk.root.path().join("build/alpha.m4b"), 95.0)
        .await;
    let book = desk.root.path().join("library/alpha.m4b");
    fs::create_dir_all(book.parent().expect("library folder")).expect("create library");
    mux_chapters(
        &built,
        &[("One", 0, 50_000), ("Two", 50_000, 95_000)],
        &book,
    );
    desk.import(&book).await;
    let library = desk.root.path().join("out");
    fs::create_dir_all(&library).expect("create output folder");
    desk.send(SessionIntent::SetOutputDirectory {
        directory: library.to_string_lossy().into_owned(),
    })
    .await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Title,
        value: "Preview Title".to_string(),
    })
    .await;

    desk.send(SessionIntent::Preview).await;
    let preview = desk
        .engine
        .session_snapshot()
        .output
        .expect("output")
        .preview_run
        .expect("preview snapshot");
    assert_eq!(preview.operation.status, WorkOperationStatus::Completed);
    let SessionOutcome::PreviewOutput { path: Some(path) } = desk
        .send(SessionIntent::TakePreviewOutput {
            run_id: preview.operation.operation_id.to_string(),
        })
        .await
    else {
        panic!("a finished preview has an output");
    };
    let path = PathBuf::from(path);

    assert_eq!(
        audio_duration_ts(&path),
        90 * u64::from(SAMPLE_RATE),
        "exactly 90 s"
    );
    assert_eq!(
        abb_engine::read_metadata(&path)
            .expect("read preview tags")
            .title
            .as_deref(),
        Some("Preview Title")
    );
    assert!(chapters_of(&path).is_empty(), "{:?}", chapters_of(&path));
}

#[tokio::test]
async fn a_preview_is_named_and_wrapped_like_the_final_file_in_every_format() {
    for (format, extension, container) in [
        (AudiobookFormat::M4b, "m4b", "mov,mp4,m4a,3gp,3g2,mj2"),
        (AudiobookFormat::M4aOpus, "m4a", "mov,mp4,m4a,3gp,3g2,mj2"),
        (AudiobookFormat::MkaOpus, "mka", "matroska,webm"),
    ] {
        let desk = Desk::new();
        let book = desk
            .audiobook(&desk.root.path().join("library/alpha.m4b"), 3.0)
            .await;
        desk.import(&book).await;
        let library = desk.root.path().join("out");
        fs::create_dir_all(&library).expect("create output folder");
        desk.send(SessionIntent::SetOutputDirectory {
            directory: library.to_string_lossy().into_owned(),
        })
        .await;
        desk.send(SessionIntent::SetField {
            binding: desk.metadata().binding,
            field: MetadataField::Title,
            value: "Vol. 2".to_string(),
        })
        .await;
        let title_ids = desk
            .engine
            .session_snapshot()
            .titles
            .expect("titles")
            .files
            .into_iter()
            .map(|file| file.input_id)
            .collect();
        desk.send(SessionIntent::SetTitleAudio {
            title_ids,
            edit: AudioEdit::Format(format),
        })
        .await;

        desk.send(SessionIntent::Preview).await;
        let preview = desk
            .engine
            .session_snapshot()
            .output
            .expect("output")
            .preview_run
            .expect("preview snapshot");
        assert_eq!(
            preview.operation.status,
            WorkOperationStatus::Completed,
            "{format:?}"
        );
        let SessionOutcome::PreviewOutput { path: Some(path) } = desk
            .send(SessionIntent::TakePreviewOutput {
                run_id: preview.operation.operation_id.to_string(),
            })
            .await
        else {
            panic!("a finished preview has an output");
        };
        let path = PathBuf::from(path);

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(format!("Vol. 2.preview.{extension}").as_str()),
            "{format:?}"
        );
        let probed = ffprobe([
            "-v",
            "error",
            "-show_entries",
            "format=format_name",
            "-of",
            "default=nw=1:nk=1",
            path.to_str().expect("utf8 path"),
        ]);
        assert_eq!(
            String::from_utf8(probed).expect("utf8").trim(),
            container,
            "{format:?}"
        );
    }
}
