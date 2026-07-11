use crate::audio::file_list::FileListInfo;
use crate::audio::settings_encoder::{
    resolve_encoder_type, validate_requested_encoder_available, EncoderSettings, EncoderType,
};
use crate::audio::toolchain::{
    detect_encoder_availability_with_resolution, validate_external_input_decoders,
    EncoderAvailability, ValidatedExternalToolchain,
};
use crate::audio::{AudioFile, DecoderSelection};
use crate::errors::{AppError, Result};
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::ProcessingContext;

#[derive(Debug, Clone)]
pub enum ResolvedProcessorAdapter {
    NativeFfmpegNext,
    ExternalFdk {
        toolchain: ValidatedExternalToolchain,
    },
}

impl ResolvedProcessorAdapter {
    pub fn validate_inputs(&self, file_info: &FileListInfo) -> Result<()> {
        match self {
            Self::NativeFfmpegNext => Ok(()),
            Self::ExternalFdk { toolchain } => validate_external_input_decoders(
                &file_info.files,
                &file_info.selected_decoders,
                toolchain,
            ),
        }
    }

    pub async fn execute(
        self,
        context: ProcessingContext,
        files: Vec<AudioFile>,
        selected_decoders: Vec<Option<DecoderSelection>>,
        metadata: Option<AudiobookMetadata>,
        cover_art_passthrough: CoverArtPassthroughPolicy,
    ) -> Result<String> {
        match self {
            Self::NativeFfmpegNext => {
                // The native pipeline (prepare -> encode -> finalize) is fully
                // synchronous, CPU-bound work. Offload it onto a blocking thread
                // so it never occupies an async runtime worker. Progress emission
                // (`window.emit`) and cooperative cancellation (atomic flag) both
                // operate correctly off the runtime.
                tokio::task::spawn_blocking(move || {
                    super::process_audiobook_with_context(
                        context,
                        files,
                        metadata,
                        cover_art_passthrough,
                    )
                })
                .await
                .map_err(|join_error| {
                    AppError::General(format!("audio processing task failed: {join_error}"))
                })?
            }
            Self::ExternalFdk { toolchain } => {
                super::external_fdk::process_audiobook_with_external_fdk(
                    context,
                    files,
                    selected_decoders,
                    metadata,
                    cover_art_passthrough,
                    toolchain,
                )
                .await
            }
        }
    }
}

pub fn resolve_processor_adapter(
    encoder_settings: &EncoderSettings,
) -> Result<ResolvedProcessorAdapter> {
    let (availability, resolution) = detect_encoder_availability_with_resolution();
    resolve_processor_adapter_from_parts(encoder_settings, &availability, resolution.validated)
}

fn resolve_processor_adapter_from_parts(
    encoder_settings: &EncoderSettings,
    availability: &EncoderAvailability,
    toolchain: Option<ValidatedExternalToolchain>,
) -> Result<ResolvedProcessorAdapter> {
    validate_requested_encoder_available(encoder_settings.encoder_type, availability)?;
    let resolved_encoder = resolve_encoder_type(encoder_settings, availability);

    if !matches!(resolved_encoder, EncoderType::FdkHeAac) {
        return Ok(ResolvedProcessorAdapter::NativeFfmpegNext);
    }

    let toolchain = toolchain.ok_or_else(|| {
        AppError::toolchain_required("FDK AAC requires a validated external FFmpeg toolchain.")
    })?;
    Ok(ResolvedProcessorAdapter::ExternalFdk { toolchain })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::settings_encoder::{BitrateMode, ChannelConfig};
    use crate::audio::toolchain::{
        EncoderCapabilitySource, ExternalDecoderCapabilities, ValidatedExternalToolchain,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn resolves_non_fdk_encoder_to_native_adapter() {
        let adapter = resolve_processor_adapter_from_parts(
            &settings(EncoderType::NativeAac),
            &availability(true, EncoderType::NativeAac),
            Some(toolchain(ExternalDecoderCapabilities {
                aac_at: true,
                libfdk_aac: true,
            })),
        )
        .expect("native adapter should resolve");

        assert!(matches!(
            adapter,
            ResolvedProcessorAdapter::NativeFfmpegNext
        ));
    }

    #[test]
    fn resolves_fdk_encoder_to_external_adapter() {
        let adapter = resolve_processor_adapter_from_parts(
            &settings(EncoderType::FdkHeAac),
            &availability(true, EncoderType::FdkHeAac),
            Some(toolchain(ExternalDecoderCapabilities {
                aac_at: true,
                libfdk_aac: true,
            })),
        )
        .expect("external adapter should resolve");

        assert!(matches!(
            adapter,
            ResolvedProcessorAdapter::ExternalFdk { .. }
        ));
    }

    #[test]
    fn rejects_fdk_encoder_without_validated_toolchain() {
        let err = resolve_processor_adapter_from_parts(
            &settings(EncoderType::FdkHeAac),
            &availability(true, EncoderType::FdkHeAac),
            None,
        )
        .expect_err("missing FDK toolchain should fail");

        assert!(err
            .to_string()
            .contains("validated external FFmpeg toolchain"));
    }

    #[test]
    fn rejects_unavailable_requested_fdk_encoder() {
        let err = resolve_processor_adapter_from_parts(
            &settings(EncoderType::FdkHeAac),
            &availability(false, EncoderType::NativeAac),
            None,
        )
        .expect_err("unavailable FDK request should fail");

        assert!(err.to_string().contains("FDK AAC"));
    }

    #[test]
    fn external_adapter_rejects_unavailable_selected_decoder() {
        let adapter = ResolvedProcessorAdapter::ExternalFdk {
            toolchain: toolchain(ExternalDecoderCapabilities {
                aac_at: false,
                libfdk_aac: true,
            }),
        };
        let file_info = FileListInfo {
            files: vec![AudioFile {
                input_id: "input-1".to_string(),
                path: Path::new("/books/input.m4b").to_path_buf(),
                size: Some(1.0),
                duration: Some(5.0),
                format: Some("M4B".to_string()),
                bitrate: None,
                sample_rate: None,
                channels: None,
                codec_label: Some("AAC".to_string()),
                selected_decoder: Some("Apple AAC".to_string()),
                chapters: Vec::new(),
                is_valid: true,
                error: None,
            }],
            selected_decoders: vec![Some(DecoderSelection {
                decoder_id: "aac_at".to_string(),
                decoder_label: "Apple AAC".to_string(),
            })],
            total_duration: 5.0,
            total_size: 1.0,
            valid_count: 1,
            invalid_count: 0,
        };

        let err = adapter
            .validate_inputs(&file_info)
            .expect_err("unsupported selected decoder should fail");

        assert!(err.to_string().contains("does not expose decoder 'aac_at'"));
    }

    fn settings(encoder_type: EncoderType) -> EncoderSettings {
        EncoderSettings {
            encoder_type,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Cbr,
            channels: ChannelConfig::Auto,
            afterburner: false,
        }
    }

    fn availability(fdk_available: bool, auto_encoder: EncoderType) -> EncoderAvailability {
        EncoderAvailability {
            fdk_available,
            fdk_source: if fdk_available {
                EncoderCapabilitySource::Detected
            } else {
                EncoderCapabilitySource::None
            },
            aac_at_available: false,
            native_aac_available: true,
            auto_encoder,
            detected_toolchain_path: None,
            status_message: String::new(),
        }
    }

    fn toolchain(decoder_capabilities: ExternalDecoderCapabilities) -> ValidatedExternalToolchain {
        ValidatedExternalToolchain {
            ffmpeg_path: PathBuf::from("/usr/local/bin/ffmpeg"),
            source: EncoderCapabilitySource::Detected,
            decoder_capabilities,
        }
    }
}
