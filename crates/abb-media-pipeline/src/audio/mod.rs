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
mod output_header;
mod path_validation;
mod processor;
mod settings;
mod settings_capabilities;
mod settings_encoder;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioFile {
    pub input_id: String,
    pub path: PathBuf,
    pub size: Option<f64>,
    pub duration: Option<f64>,
    pub format: Option<String>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub codec_label: Option<String>,
    #[serde(skip)]
    pub preservation: Option<AudioPreservation>,
    pub selected_decoder: Option<String>,
    pub label: String,
    pub artist: Option<String>,
    #[serde(skip)]
    pub tag_title: Option<String>,
    pub chapters: Vec<AudioChapter>,
    pub chapter_plan: Option<crate::metadata::ChapterPlan>,
    pub cue_source: Option<crate::metadata::CueSource>,
    pub is_valid: bool,
    pub source_removed: bool,
    pub status: TitleStatus,
    pub error: Option<String>,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum TitleStatus {
    Ready,
    Failed,
    RemovedFinished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPreservation {
    pub can_preserve: bool,
    pub(crate) codec_family: Option<CodecFamily>,
}

pub type AudioChapter = crate::metadata::ChapterSpec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DecoderSelection {
    pub decoder_id: String,
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

    pub fn set_tag_title(&mut self, title: Option<String>) {
        self.tag_title = title;
        self.relabel();
    }

    pub fn set_artist(&mut self, artist: Option<String>) {
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

    pub fn takes_edits(&self) -> bool {
        self.is_valid || self.source_removed
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SampleRateConfig {
    Auto,
    Explicit(u32),
}

pub use ffmpeg_log::limit_ffmpeg_logging;
pub use file_list::{apply_chapter_plans, get_file_list_info, FileListInfo};
pub use format::CodecFamily;
pub(crate) use format::{
    format_for_extension, is_matroska_format, is_mp4_family_format, muxer_for_path,
};
pub use format::{Muxer, FORMATS};
pub use imports::discover_audio_import_paths;
pub use imports::{
    supported_audio_import_extensions, supported_audio_import_formats_text,
    supported_audio_import_support_text, SkippedImportPath,
};
pub use output_header::write_output_header;
pub use path_validation::{validate_input_audio_path, validate_input_image_path};
pub use processor::probe_builtin_audio_decoder;
pub(crate) use processor::{
    codec_library_versions, execute_audio_engine, AudioExecutionRequest, AudioRunOutcome,
};
pub(crate) use processor::{passthrough_sources_from_audio_files, validate_preserved_title};
#[cfg(test)]
pub(crate) use settings::automatic_sample_rate;
pub use settings::check_output_directory;
pub(crate) use settings::validate_output_target;
pub use settings::validate_sample_rate_config;

impl AudioFile {
    pub fn own_status(&self) -> TitleStatus {
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
pub use settings_capabilities::encoder_settings_capabilities;
pub use settings_capabilities::{
    EncoderAvailability, EncoderConfigurationCapability, EncoderSettingsCapabilities,
    FaacProfileCapability,
};
pub use settings_encoder::{validate_encoder_settings, DEFAULT_FAAC_QUALITY};
pub use settings_encoder::{
    BitrateMode, BitrateModeKind, ChannelConfig, EncoderSettings, EncoderType, FaacProfile,
};

pub(crate) use processor::processing_workspace_root;

pub fn cleanup_abandoned_processing_workspaces(cache_dir: &Path) -> crate::errors::Result<()> {
    processor::cleanup_abandoned_processing_workspaces(cache_dir)
}

#[cfg(test)]
mod contract_tests;

mod output_plan;
pub use output_plan::{
    AudioHandling, AudioIntent, AudioPlanField, AudiobookFormat, OutputAudio, TitleAudioError,
    TitleAudioPlan, TitleAudioRequest,
};

pub use output_plan::resolve_title_audio;
