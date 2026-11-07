use super::*;
use crate::audio::{AudiobookFormat, EncoderSettings};
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::output_artifact::{OutputCollisionKind, PlannedOutputAction, ResolvedOutputPlan};
use crate::processing::plan::PlannedProcessingJob;
use abb_output_artifact_core::OutputCollision;
use std::path::PathBuf;

fn job() -> PlannedProcessingJob {
    PlannedProcessingJob {
        input_index: 0,
        input_path: PathBuf::from("/in/a.m4b"),
        source_paths: vec![PathBuf::from("/in/a.m4b")],
        output: ResolvedOutputPlan {
            kind: OutputKind::Final,
            requested_path: PathBuf::from("/out/A.m4b"),
            resolved_path: PathBuf::from("/out/A.m4b"),
            rename_candidate: None,
            collision: None,
            action: PlannedOutputAction::Write,
        },
        metadata: None,
        source_metadata: AudiobookMetadata::default(),
        cover_art_passthrough: CoverArtPassthroughPolicy::Preserve,
        audio_plan: TitleAudioPlan {
            format: AudiobookFormat::M4b,
            handling: AudioHandling::Encode {
                settings: EncoderSettings::default(),
            },
            sample_rate: 44_100,
            channels: 2,
            source_codec: "mp3".into(),
            reason: None,
        },
        metadata_intent: None,
    }
}

fn signature(
    policy: CollisionPolicy,
    decoder: Option<AacDecoder>,
    job: PlannedProcessingJob,
) -> String {
    build(OutputKind::Final, policy, decoder, &[job])
}

fn base() -> String {
    signature(CollisionPolicy::Fail, Some(AacDecoder::Auto), job())
}

#[test]
fn the_same_plan_signs_the_same() {
    assert_eq!(base(), base());
}

#[test]
fn every_assumption_a_review_was_made_against_changes_the_signature() {
    let changed: Vec<(&str, String)> = vec![
        (
            "policy",
            signature(
                CollisionPolicy::ReplaceExisting,
                Some(AacDecoder::Auto),
                job(),
            ),
        ),
        ("decoder", signature(CollisionPolicy::Fail, None, job())),
        (
            "preview",
            build(
                OutputKind::Preview,
                CollisionPolicy::Fail,
                Some(AacDecoder::Auto),
                &[job()],
            ),
        ),
        ("source", {
            let mut other = job();
            other.source_paths.push(PathBuf::from("/in/b.m4b"));
            signature(CollisionPolicy::Fail, Some(AacDecoder::Auto), other)
        }),
        ("destination", {
            let mut other = job();
            other.output.resolved_path = PathBuf::from("/out/B.m4b");
            signature(CollisionPolicy::Fail, Some(AacDecoder::Auto), other)
        }),
        ("bitrate", {
            let mut other = job();
            other.audio_plan.handling = AudioHandling::Encode {
                settings: EncoderSettings {
                    bitrate_kbps: 96,
                    ..EncoderSettings::default()
                },
            };
            signature(CollisionPolicy::Fail, Some(AacDecoder::Auto), other)
        }),
        ("handling", {
            let mut other = job();
            other.audio_plan.handling = AudioHandling::Preserve;
            signature(CollisionPolicy::Fail, Some(AacDecoder::Auto), other)
        }),
        ("collision", {
            let mut other = job();
            other.output.collision = Some(OutputCollision {
                kind: OutputCollisionKind::ExistingFile,
                conflicting_path: Some(PathBuf::from("/out/A.m4b")),
                detail: None,
            });
            other.output.action = PlannedOutputAction::ReviewRequired;
            signature(CollisionPolicy::Fail, Some(AacDecoder::Auto), other)
        }),
    ];
    for (name, other) in &changed {
        assert_ne!(*other, base(), "{name}");
    }
    for (index, (left, one)) in changed.iter().enumerate() {
        for (right, two) in &changed[index + 1..] {
            assert_ne!(one, two, "{left} and {right}");
        }
    }
}

#[test]
fn moving_a_path_between_two_fields_changes_the_signature() {
    let mut joined = job();
    joined.source_paths = vec![PathBuf::from("/in/a.m4b/in/b.m4b")];
    let mut split = job();
    split.source_paths = vec![PathBuf::from("/in/a.m4b"), PathBuf::from("/in/b.m4b")];
    assert_ne!(
        signature(CollisionPolicy::Fail, None, joined),
        signature(CollisionPolicy::Fail, None, split)
    );
}
