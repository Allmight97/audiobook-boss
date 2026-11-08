use super::plan::{collision_kind_label, collision_path_label, PlannedProcessingJob};
use crate::audio::{AacDecoder, AudioHandling, EncoderSettings, TitleAudioPlan};
use crate::output_artifact::{CollisionPolicy, OutputKind};

/// Length-prefixed fields, so two different plans never encode the same bytes.
#[derive(Default)]
struct SignatureInput(Vec<u8>);

impl SignatureInput {
    fn bytes(&mut self, value: &[u8]) {
        self.0
            .extend_from_slice(&(value.len() as u64).to_le_bytes());
        self.0.extend_from_slice(value);
    }

    fn text(&mut self, value: impl std::fmt::Display) {
        self.bytes(value.to_string().as_bytes());
    }

    fn path(&mut self, value: &std::path::Path) {
        self.bytes(value.as_os_str().as_encoded_bytes());
    }

    fn audio_plan(&mut self, plan: &TitleAudioPlan) {
        let TitleAudioPlan {
            format,
            handling,
            sample_rate,
            channels,
            source_codec,
            reason,
        } = plan;
        self.text(format!("{format:?}"));
        match handling {
            AudioHandling::Preserve => self.text("preserve"),
            AudioHandling::Encode { settings } => {
                self.text("encode");
                self.encoder_settings(settings);
            }
        }
        self.text(sample_rate);
        self.text(channels);
        self.text(source_codec);
        self.text(reason.as_deref().unwrap_or_default());
    }

    fn encoder_settings(&mut self, settings: &EncoderSettings) {
        let EncoderSettings {
            encoder_type,
            bitrate_kbps,
            bitrate_mode,
            channels,
            native_aac_speed,
            faac_profile,
        } = settings;
        self.text(format!("{encoder_type:?}"));
        self.text(bitrate_kbps);
        self.text(format!("{bitrate_mode:?}"));
        self.text(format!("{channels:?}"));
        self.text(native_aac_speed);
        self.text(format!("{faac_profile:?}"));
    }
}

/// Identifies the destination and audio assumptions a collision review was
/// made against; execution refuses a review whose signature no longer matches.
pub(super) fn build(
    output_kind: OutputKind,
    collision_policy: CollisionPolicy,
    aac_decoder: Option<AacDecoder>,
    jobs: &[PlannedProcessingJob],
) -> String {
    let mut input = SignatureInput::default();
    input.text(format!("{output_kind:?}"));
    input.text(format!("{collision_policy:?}"));
    input.text(format!("{aac_decoder:?}"));
    for job in jobs {
        input.text(job.input_index);
        input.text(job.source_paths.len());
        for source in &job.source_paths {
            input.path(source);
        }
        input.audio_plan(&job.audio_plan);
        let output = &job.output;
        input.path(&output.requested_path);
        input.path(&output.resolved_path);
        input.path(
            output
                .rename_candidate
                .as_deref()
                .unwrap_or(std::path::Path::new("")),
        );
        input.text(format!("{:?}", output.kind));
        input.text(format!("{:?}", output.action));
        input.text(collision_kind_label(output));
        input.text(collision_path_label(output));
        input.text(
            output
                .collision
                .as_ref()
                .and_then(|value| value.detail.as_deref())
                .unwrap_or_default(),
        );
    }
    abb_media_core::sha256_hex(&input.0)
}

#[cfg(test)]
#[path = "plan_signature_tests.rs"]
mod tests;
