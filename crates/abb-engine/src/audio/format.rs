use super::{AudiobookFormat, EncoderType};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CodecFamily {
    Aac,
    Opus,
}

impl CodecFamily {
    pub(crate) const fn of_codec_id(id: ffmpeg_next::codec::Id) -> Option<Self> {
        match id {
            ffmpeg_next::codec::Id::AAC => Some(Self::Aac),
            ffmpeg_next::codec::Id::OPUS => Some(Self::Opus),
            _ => None,
        }
    }

    pub(crate) const fn codec_id(self) -> ffmpeg_next::codec::Id {
        match self {
            Self::Aac => ffmpeg_next::codec::Id::AAC,
            Self::Opus => ffmpeg_next::codec::Id::OPUS,
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

pub(crate) fn codec_family_of(encoder: EncoderType) -> CodecFamily {
    FORMATS
        .into_iter()
        .find(|spec| spec.encoders.contains(&encoder))
        .map(|spec| spec.codec_family)
        .expect("every encoder type belongs to an output format")
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
