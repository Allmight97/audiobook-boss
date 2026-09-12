use crate::audio::file_list::FileListInfo;
use crate::audio::settings_encoder::{
    is_encoder_available_by_name, resolve_encoder_name, resolve_encoder_type,
    validate_encoder_available, validate_requested_encoder_available, ChannelConfig,
    EncoderSettings, EncoderType,
};
use crate::audio::toolchain::{
    detect_encoder_availability_with_resolution, validate_external_input_decoders,
    EncoderAvailability, ValidatedExternalToolchain,
};
use crate::audio::{AudioFile, DecoderSelection};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::ProcessingContext;

#[derive(Debug, Clone)]
pub enum ResolvedProcessorAdapter {
    NativeFfmpegNext {
        encoder_type: EncoderType,
    },
    ExternalFdk {
        toolchain: ValidatedExternalToolchain,
    },
}

pub(super) fn resolve_output_channels(
    requested: ChannelConfig,
    files: &[AudioFile],
) -> Result<ChannelConfig> {
    if requested != ChannelConfig::Auto {
        return Ok(requested);
    }

    let mut resolved = None;
    for file in files.iter().filter(|file| file.is_valid) {
        match file.channels {
            Some(1) => {
                resolved.get_or_insert(ChannelConfig::Mono);
            }
            Some(2) => resolved = Some(ChannelConfig::Stereo),
            Some(channels) if channels > 2 => {
                return Err(AppError::InvalidInput(format!(
                    "'{}' has {channels} audio channels. Choose Mono or Stereo to downmix multichannel audio.",
                    sanitize_path_for_display(&file.path)
                )));
            }
            _ => {
                return Err(AppError::InvalidInput(format!(
                    "Could not determine audio channels for '{}'. Choose Mono or Stereo explicitly.",
                    sanitize_path_for_display(&file.path)
                )));
            }
        }
    }
    resolved.ok_or_else(|| AppError::InvalidInput("No valid audio files to process.".into()))
}

impl ResolvedProcessorAdapter {
    pub fn validate_inputs(&self, file_info: &FileListInfo) -> Result<()> {
        match self {
            Self::NativeFfmpegNext { .. } => Ok(()),
            Self::ExternalFdk { toolchain } => validate_external_input_decoders(
                &file_info.files,
                &file_info.selected_decoders,
                toolchain,
            ),
        }
    }

    pub async fn execute(
        self,
        mut context: ProcessingContext,
        files: Vec<AudioFile>,
        selected_decoders: Vec<Option<DecoderSelection>>,
        metadata: Option<AudiobookMetadata>,
        cover_art_passthrough: CoverArtPassthroughPolicy,
    ) -> Result<String> {
        match self {
            Self::NativeFfmpegNext { encoder_type } => {
                context.encoder_settings.encoder_type = encoder_type;
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
    let requested = encoder_settings.encoder_type;
    if matches!(requested, EncoderType::NativeAac | EncoderType::AacAt) {
        let platform_supported = requested != EncoderType::AacAt || cfg!(target_os = "macos");
        let available = platform_supported
            && if requested == EncoderType::NativeAac {
                crate::audio::settings_encoder::is_native_nmr_available()
            } else {
                is_encoder_available_by_name(resolve_encoder_name(requested))
            };
        validate_encoder_available(requested, available)?;
        return Ok(ResolvedProcessorAdapter::NativeFfmpegNext {
            encoder_type: requested,
        });
    }
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
    validate_requested_encoder_available(resolved_encoder, availability)?;
    let mut resolved_settings = encoder_settings.clone();
    resolved_settings.encoder_type = resolved_encoder;
    crate::audio::settings_encoder::validate_encoder_settings(&resolved_settings)?;

    if !matches!(resolved_encoder, EncoderType::FdkHeAac) {
        return Ok(ResolvedProcessorAdapter::NativeFfmpegNext {
            encoder_type: resolved_encoder,
        });
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

    fn channel_files(counts: &[Option<u32>]) -> Vec<AudioFile> {
        counts
            .iter()
            .map(|channels| {
                let mut file = AudioFile::new("input.wav".into());
                file.channels = *channels;
                file.is_valid = true;
                file
            })
            .collect()
    }

    #[test]
    fn auto_channels_preserve_stereo_in_either_input_order() {
        for counts in [[Some(1), Some(2)], [Some(2), Some(1)]] {
            assert_eq!(
                resolve_output_channels(ChannelConfig::Auto, &channel_files(&counts))
                    .expect("resolve mixed channels"),
                ChannelConfig::Stereo
            );
        }
        assert_eq!(
            resolve_output_channels(ChannelConfig::Auto, &channel_files(&[Some(1), Some(1)]))
                .expect("resolve mono inputs"),
            ChannelConfig::Mono
        );
    }

    #[test]
    fn auto_channels_require_an_explicit_downmix_for_multichannel_or_unknown_inputs() {
        for channels in [Some(6), Some(0), None] {
            let files = channel_files(&[Some(2), channels]);
            let error = resolve_output_channels(ChannelConfig::Auto, &files)
                .expect_err("Auto requires known mono/stereo inputs");
            assert!(error.to_string().contains("Choose Mono or Stereo"));
            for forced in [ChannelConfig::Mono, ChannelConfig::Stereo] {
                assert_eq!(
                    resolve_output_channels(forced, &files).expect("explicit downmix accepted"),
                    forced
                );
            }
        }
    }

    #[test]
    fn auto_channels_only_resolve_valid_inputs() {
        let mut files = channel_files(&[Some(1), Some(6)]);
        files[1].is_valid = false;
        assert_eq!(
            resolve_output_channels(ChannelConfig::Auto, &files).expect("resolve mono inputs"),
            ChannelConfig::Mono
        );
        files[0].is_valid = false;
        assert!(resolve_output_channels(ChannelConfig::Auto, &files).is_err());
    }

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
            ResolvedProcessorAdapter::NativeFfmpegNext {
                encoder_type: EncoderType::NativeAac
            }
        ));
    }

    #[test]
    fn auto_resolution_validates_the_requested_mode_for_the_resolved_encoder() {
        let available = availability(false, EncoderType::NativeAac);
        let mut requested = settings(EncoderType::Auto);
        let adapter = resolve_processor_adapter_from_parts(&requested, &available, None)
            .expect("auto target request resolves to available Native AAC");
        assert!(matches!(
            adapter,
            ResolvedProcessorAdapter::NativeFfmpegNext {
                encoder_type: EncoderType::NativeAac
            }
        ));

        requested.bitrate_mode = BitrateMode::Vbr(3);
        let error = resolve_processor_adapter_from_parts(&requested, &available, None)
            .expect_err("stale FDK quality intent cannot become native target encoding");
        assert!(matches!(error, AppError::InvalidInput(_)));
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
                chapter_plan: None,
                cue_source: None,
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
                tag_title: None,
                tag_artist: None,
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
            bitrate_mode: if encoder_type == EncoderType::FdkHeAac {
                BitrateMode::Vbr(3)
            } else {
                BitrateMode::Cbr
            },
            channels: ChannelConfig::Auto,
            afterburner: false,
            native_aac_speed: 0,
        }
    }

    fn availability(fdk_available: bool, auto_encoder: EncoderType) -> EncoderAvailability {
        EncoderAvailability {
            fdk_setup_supported: true,
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
