use crate::errors::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

mod buffer;
mod cleanup;
mod constants;
pub use constants::ALLOWED_IMAGE_EXTENSIONS;
mod extensions;
mod ffmpeg_log;
mod file_list;
mod format;
mod imports;
mod path_validation;
mod processor;
mod settings;
mod settings_capabilities;
mod settings_encoder;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioFile {
    /// Stable workbench/session identity for joins that must survive reorder/remove operations.
    pub input_id: String,
    pub path: PathBuf,
    /// Bytes.
    pub size: Option<f64>,
    /// Seconds.
    pub duration: Option<f64>,
    pub format: Option<String>,
    /// Bits per second.
    pub bitrate: Option<u32>,
    /// Hz.
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub codec_label: Option<String>,
    /// Whether this source can be copied without re-encoding and whether the
    /// source meets the automatic low-bitrate recommendation.
    #[serde(skip)]
    pub preservation: Option<AudioPreservation>,
    /// Display only.
    pub selected_decoder: Option<String>,
    /// What the title list calls this source: the title tag, else the file name.
    pub label: String,
    pub artist: Option<String>,
    #[serde(skip)]
    pub(crate) tag_title: Option<String>,
    /// Chapters embedded in this individual source file, normalized to milliseconds.
    pub chapters: Vec<AudioChapter>,
    pub chapter_plan: Option<crate::metadata::ChapterPlan>,
    pub cue_source: Option<crate::metadata::CueSource>,
    pub is_valid: bool,
    /// An exported download that ABB removed. The title is finished, not failed.
    pub source_removed: bool,
    /// Whether this entry can be exported, set by the session on the titles
    /// and sources it lists: a title takes its sources' combined status.
    pub status: TitleStatus,
    pub error: Option<String>,
    /// The address of this source's embedded cover, set by the session on
    /// the sources it lists; absent when the source is not valid.
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum TitleStatus {
    Ready,
    Failed,
    /// An exported download that ABB removed. The title is finished, not failed.
    RemovedFinished,
}

/// What inspection learned about copying a source: whether its container and
/// codec can be copied unchanged, and the codec family a format must share to
/// keep it. `None` is a codec no output format holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPreservation {
    pub can_preserve: bool,
    pub(crate) codec_family: Option<CodecFamily>,
}

pub type AudioChapter = crate::metadata::ChapterSpec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DecoderSelection {
    /// Stable decoder identifier used for routing and comparisons.
    pub decoder_id: String,
    /// Friendly decoder label used for display only.
    pub decoder_label: String,
}

impl fmt::Display for DecoderSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.decoder_label)
    }
}

impl AudioFile {
    pub fn new(path: PathBuf) -> Self {
        let mut file = Self {
            input_id: uuid::Uuid::new_v4().to_string(),
            path,
            size: None,
            duration: None,
            format: None,
            bitrate: None,
            sample_rate: None,
            channels: None,
            codec_label: None,
            preservation: None,
            selected_decoder: None,
            label: String::new(),
            artist: None,
            tag_title: None,
            chapters: Vec::new(),
            chapter_plan: None,
            cue_source: None,
            is_valid: false,
            source_removed: false,
            status: TitleStatus::Failed,
            cover: None,
            error: None,
        };
        file.relabel();
        file
    }

    pub(crate) fn set_path(&mut self, path: PathBuf) {
        self.path = path;
        self.relabel();
    }

    pub(crate) fn set_tag_title(&mut self, title: Option<String>) {
        self.tag_title = title;
        self.relabel();
    }

    pub(crate) fn set_artist(&mut self, artist: Option<String>) {
        self.artist = artist
            .map(|artist| artist.trim().to_string())
            .filter(|artist| !artist.is_empty());
    }

    fn relabel(&mut self) {
        self.label = self
            .tag_title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_string)
            .or_else(|| {
                self.path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned());
    }

    pub(crate) fn takes_edits(&self) -> bool {
        self.is_valid || self.source_removed
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SampleRateConfig {
    /// Automatically detect from input files
    Auto,
    /// Explicit sample rate in Hz
    Explicit(u32),
}

pub(crate) use ffmpeg_log::limit_ffmpeg_logging;
pub(crate) use file_list::{apply_chapter_plans, get_file_list_info, FileListInfo};
pub use format::CodecFamily;
pub(crate) use format::{codec_family_of, format_for_extension, Muxer, FORMATS};
pub(crate) use imports::discover_audio_import_paths;
pub use imports::{
    supported_audio_import_extensions, supported_audio_import_formats_text,
    supported_audio_import_support_text, SkippedImportPath,
};
pub(crate) use path_validation::{validate_input_audio_path, validate_input_image_path};
pub(crate) use processor::{
    codec_library_versions, execute_audio_engine, probe_builtin_audio_decoder,
    AudioExecutionRequest, AudioRunOutcome,
};
pub(crate) use processor::{passthrough_sources_from_audio_files, validate_preserved_title};
#[cfg(test)]
pub(crate) use settings::automatic_sample_rate;
pub(crate) use settings::validate_sample_rate_config;
pub(crate) use settings::{check_output_directory, validate_output_target};

impl AudioFile {
    pub(crate) fn own_status(&self) -> TitleStatus {
        if self.source_removed {
            TitleStatus::RemovedFinished
        } else if self.is_valid {
            TitleStatus::Ready
        } else {
            TitleStatus::Failed
        }
    }
}

pub(crate) fn validate_preservation_source(file: &AudioFile) -> Result<()> {
    if !file.is_valid || !file.preservation.is_some_and(|value| value.can_preserve) {
        return Err(AppError::InvalidInput(
            "Source audio is not a supported AAC or Opus container for preservation.".to_string(),
        ));
    }
    Ok(())
}
pub use crate::execution::AacDecoder;
pub(crate) use settings_capabilities::encoder_settings_capabilities;
pub use settings_capabilities::{
    EncoderAvailability, EncoderConfigurationCapability, EncoderSettingsCapabilities,
    FaacProfileCapability,
};
pub(crate) use settings_encoder::{validate_encoder_settings, DEFAULT_FAAC_QUALITY};
pub use settings_encoder::{
    BitrateMode, BitrateModeKind, ChannelConfig, EncoderSettings, EncoderType, FaacProfile,
};

pub(crate) fn processing_workspace_root(cache_dir: &Path) -> PathBuf {
    processor::processing_workspace_root(cache_dir)
}

pub(crate) fn cleanup_abandoned_processing_workspaces(
    cache_dir: &Path,
) -> crate::errors::Result<()> {
    processor::cleanup_abandoned_processing_workspaces(cache_dir)
}

#[cfg(test)]
mod contract_tests;

mod output_plan;
pub use output_plan::{
    AudioHandling, AudioIntent, AudioPlanField, AudiobookFormat, OutputAudio, TitleAudioError,
    TitleAudioPlan, TitleAudioRequest,
};

pub(crate) use output_plan::resolve_title_audio;
