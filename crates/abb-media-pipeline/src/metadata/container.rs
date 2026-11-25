use std::path::Path;

use super::field_schema::{ID3_ALBUM_SORT_KEY, MP4_ALBUM_SORT_KEY};
use crate::audio::Muxer;
use crate::errors::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Container {
    Mp4,
    Matroska,
    Mp3,
    Adts,
    Wav,
    Other(String),
}

impl From<Muxer> for Container {
    fn from(muxer: Muxer) -> Self {
        match muxer {
            Muxer::Mp4 => Container::Mp4,
            Muxer::Matroska => Container::Matroska,
        }
    }
}

impl Container {
    pub(crate) fn from_format_name(name: &str) -> Self {
        if crate::audio::is_mp4_family_format(name) {
            return Container::Mp4;
        }
        match name.split(',').next().unwrap_or(name) {
            "matroska" => Container::Matroska,
            "mp3" => Container::Mp3,
            "aac" | "adts" => Container::Adts,
            "wav" => Container::Wav,
            other => Container::Other(other.to_string()),
        }
    }

    pub(crate) fn of_input(input: &abb_ffmpeg::Input) -> Self {
        Self::from_format_name(&input.format_name())
    }

    pub(crate) fn of_output(output: &abb_ffmpeg::Output) -> Self {
        Self::from_format_name(&output.muxer_name())
    }

    pub(crate) fn from_extension(extension: &str) -> Option<Self> {
        crate::audio::format_for_extension(extension).map(|spec| spec.muxer.into())
    }

    pub(crate) fn is_mp4_family(&self) -> bool {
        *self == Container::Mp4
    }

    pub(crate) fn uses_vorbis_comments(&self) -> bool {
        *self == Container::Other("flac".to_string())
    }

    pub(crate) fn stores_chapter_ends(&self) -> bool {
        !self.is_mp4_family()
    }

    pub(crate) fn album_sort_key(&self) -> &'static str {
        match self {
            Container::Mp3 => ID3_ALBUM_SORT_KEY,
            _ => MP4_ALBUM_SORT_KEY,
        }
    }

    pub(crate) fn muxer_name(&self) -> &str {
        match self {
            Container::Mp4 => Muxer::Mp4.ffmpeg_name(),
            Container::Matroska => Muxer::Matroska.ffmpeg_name(),
            Container::Mp3 => "mp3",
            Container::Adts => "adts",
            Container::Wav => "wav",
            Container::Other(name) => name,
        }
    }
}

pub(crate) fn classify(path: &Path) -> Result<Container> {
    let format_name = crate::metadata::open_input(path)?.format_name();
    let container = Container::from_format_name(&format_name);
    log::info!(
        "metadata_route artifact={} input_format={} route={container:?}",
        crate::diagnostics::artifact_id(path),
        format_name
    );
    Ok(container)
}

#[cfg(test)]
#[path = "container_tests.rs"]
mod tests;
