use super::{AudiobookFormat, EncoderType};
use crate::errors::{AppError, Result};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Muxer {
    Mp4,
    Matroska,
}

impl Muxer {
    pub(crate) const fn ffmpeg_name(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Matroska => "matroska",
        }
    }
}

const MP4_FAMILY_NAMES: [&str; 6] = ["mov", "mp4", "m4a", "3gp", "3g2", "mj2"];

/// Whether an FFmpeg format name is in the MP4 family. `name` is
/// `format().name()`: a muxer name, or a demuxer's comma-separated aliases
/// (`mov,mp4,m4a,3gp,3g2,mj2`).
pub(crate) fn is_mp4_family_format(name: &str) -> bool {
    name.split(',')
        .any(|alias| MP4_FAMILY_NAMES.contains(&alias))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CodecFamily {
    Aac,
    Opus,
}

impl CodecFamily {
    pub(crate) fn of_codec_id(id: abb_ffmpeg::CodecId) -> Option<Self> {
        match id {
            abb_ffmpeg::CodecId::AAC => Some(Self::Aac),
            abb_ffmpeg::CodecId::OPUS => Some(Self::Opus),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FormatSpec {
    pub(crate) format: AudiobookFormat,
    pub(crate) extension: &'static str,
    pub(crate) muxer: Muxer,
    pub(crate) codec_family: CodecFamily,
    pub(crate) encoders: &'static [EncoderType],
}

impl FormatSpec {
    pub(crate) fn fixed_encoder(&self) -> Option<EncoderType> {
        match self.encoders {
            [only] => Some(*only),
            _ => None,
        }
    }
}

const M4B: FormatSpec = FormatSpec {
    format: AudiobookFormat::M4b,
    extension: "m4b",
    muxer: Muxer::Mp4,
    codec_family: CodecFamily::Aac,
    encoders: &[
        EncoderType::Auto,
        EncoderType::NativeAac,
        EncoderType::AacAt,
        EncoderType::Faac,
    ],
};

const M4A_OPUS: FormatSpec = FormatSpec {
    format: AudiobookFormat::M4aOpus,
    extension: "m4a",
    muxer: Muxer::Mp4,
    codec_family: CodecFamily::Opus,
    encoders: &[EncoderType::Opus],
};

const MKA_OPUS: FormatSpec = FormatSpec {
    format: AudiobookFormat::MkaOpus,
    extension: "mka",
    muxer: Muxer::Matroska,
    codec_family: CodecFamily::Opus,
    encoders: &[EncoderType::Opus],
};

pub(crate) const FORMATS: [&FormatSpec; 3] = [&M4B, &M4A_OPUS, &MKA_OPUS];

impl AudiobookFormat {
    pub(crate) const fn spec(self) -> &'static FormatSpec {
        match self {
            Self::M4b => &M4B,
            Self::M4aOpus => &M4A_OPUS,
            Self::MkaOpus => &MKA_OPUS,
        }
    }
}

pub(crate) fn format_for_extension(extension: &str) -> Option<&'static FormatSpec> {
    FORMATS.into_iter().find(|spec| spec.extension == extension)
}

pub(crate) fn muxer_for_path(path: &Path) -> Result<Muxer> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(format_for_extension)
        .map(|spec| spec.muxer)
        .ok_or_else(|| AppError::InvalidInput("Unsupported audiobook container.".into()))
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
