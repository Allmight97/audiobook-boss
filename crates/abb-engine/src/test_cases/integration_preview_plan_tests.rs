//! A preview is planned beside the finished book, never over it: its output
//! is `<name>.preview.<ext>`, whatever the collision policy, and exporting it
//! leaves the book that is already there alone.

use std::fs;
use std::path::{Path, PathBuf};

use crate::audio::{
    AudioIntent, AudiobookFormat, BitrateMode, ChannelConfig, EncoderSettings, EncoderType,
    OutputAudio, SampleRateConfig, TitleAudioRequest,
};
use crate::output_artifact::{CollisionPolicy, OutputKind, PlannedOutputAction};
use crate::session::{MetadataField, SessionIntent, SessionOutcome};
use crate::work_runtime::WorkOperationStatus;

use super::integration_session_desk::Desk;
use super::media_fixtures::{
    native_encoder_settings, preflight_payload, single_title_payload, MediaLane,
};

const FINISHED_BOOK: &[u8] = b"the finished book the user already has";

fn output_for(format: AudiobookFormat) -> OutputAudio {
    match format {
        AudiobookFormat::M4b => OutputAudio::M4b {
            settings: native_encoder_settings(),
        },
        other => OutputAudio::new(
            other,
            EncoderSettings {
                encoder_type: EncoderType::Opus,
                bitrate_mode: BitrateMode::VbrTarget,
                channels: ChannelConfig::Auto,
                ..native_encoder_settings()
            },
        ),
    }
}

fn payload(
    source: &Path,
    out_dir: &Path,
    format: AudiobookFormat,
    policy: CollisionPolicy,
) -> crate::processing::ProcessPayload {
    let mut payload = single_title_payload(
        source,
        out_dir,
        TitleAudioRequest {
            output: output_for(format),
            intent: AudioIntent::Encode,
            sample_rate: SampleRateConfig::Auto,
        },
    );
    payload.collision_policy = Some(policy);
    payload
}

fn planned_output(
    source: &Path,
    out_dir: &Path,
    format: AudiobookFormat,
    policy: CollisionPolicy,
    preview: bool,
) -> crate::output_artifact::PlannedOutput {
    preflight_payload(payload(source, out_dir, format, policy), None, preview)
        .expect("plan the title")
        .outputs
        .remove(0)
}

fn preview_name(final_path: &Path) -> PathBuf {
    let extension = final_path.extension().expect("extension");
    final_path.with_extension(format!("preview.{}", extension.to_string_lossy()))
}

#[test]
fn a_preview_plan_is_the_preview_name_beside_a_finished_book_under_every_policy() {
    let policies = [
        CollisionPolicy::Fail,
        CollisionPolicy::ReplaceExisting,
        CollisionPolicy::RenameNew,
        CollisionPolicy::SkipExisting,
    ];
    for format in [
        AudiobookFormat::M4b,
        AudiobookFormat::M4aOpus,
        AudiobookFormat::MkaOpus,
    ] {
        let lane = MediaLane::with_fixtures(&[1.0]);
        let source = &lane.inputs[0];
        let out_dir = lane.tmp.path().join("out");
        let book = PathBuf::from(
            planned_output(source, &out_dir, format, CollisionPolicy::Fail, false).resolved_path,
        );
        fs::create_dir_all(book.parent().expect("book folder")).expect("book folder");
        fs::write(&book, FINISHED_BOOK).expect("the finished book");

        for policy in policies {
            let label = format!("{format:?} under {policy:?}");
            let output = planned_output(source, &out_dir, format, policy, true);
            assert_eq!(output.kind, OutputKind::Preview, "{label}");
            assert_eq!(
                PathBuf::from(&output.resolved_path),
                preview_name(&book),
                "{label}"
            );
            assert_eq!(output.action, PlannedOutputAction::Write, "{label}");
            assert!(output.collision.is_none(), "{label}");
        }
        assert_eq!(fs::read(&book).expect("the book"), FINISHED_BOOK);
    }
}

#[test]
fn a_stale_preview_is_the_only_file_a_preview_plan_may_replace() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let source = &lane.inputs[0];
    let out_dir = lane.tmp.path().join("out");
    let format = AudiobookFormat::M4b;
    let book = PathBuf::from(
        planned_output(source, &out_dir, format, CollisionPolicy::Fail, false).resolved_path,
    );
    fs::create_dir_all(book.parent().expect("book folder")).expect("book folder");
    fs::write(&book, FINISHED_BOOK).expect("the finished book");
    fs::write(preview_name(&book), b"an old preview").expect("a stale preview");

    let output = planned_output(
        source,
        &out_dir,
        format,
        CollisionPolicy::ReplaceExisting,
        true,
    );

    assert_eq!(output.action, PlannedOutputAction::ReplaceExisting);
    assert_eq!(PathBuf::from(&output.resolved_path), preview_name(&book));
    assert_eq!(fs::read(&book).expect("the book"), FINISHED_BOOK);
}

#[tokio::test]
async fn exporting_a_preview_leaves_the_finished_book_beside_it_alone() {
    let desk = Desk::new();
    let built = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 3.0)
        .await;
    desk.import(&built).await;
    let folder = desk.root.path().join("out/Source Author/Vol. 2");
    fs::create_dir_all(&folder).expect("output folder");
    let book = folder.join("Vol. 2.m4b");
    fs::write(&book, FINISHED_BOOK).expect("the finished book");
    desk.send(SessionIntent::SetOutputDirectory {
        directory: desk.root.path().join("out").to_string_lossy().into_owned(),
    })
    .await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Title,
        value: "Vol. 2".to_string(),
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

    assert_eq!(PathBuf::from(path), folder.join("Vol. 2.preview.m4b"));
    assert_eq!(fs::read(&book).expect("the book"), FINISHED_BOOK);
    assert!(desk.residual_workspace_dirs().is_empty());
}
