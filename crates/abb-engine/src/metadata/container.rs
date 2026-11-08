use std::path::Path;

use ffmpeg_next as ff;

use super::field_schema::{ID3_ALBUM_SORT_KEY, MP4_ALBUM_SORT_KEY};
use crate::audio::Muxer;
use crate::errors::{AppError, Result};

/// The container of an audio file, named once from an FFmpeg demuxer or muxer
/// name or from an extension. `Other` holds the muxer name that writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Container {
    Mp4,
    Matroska,
    Mp3,
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
    /// `name` is FFmpeg's `format().name()`: a muxer name, or a demuxer's
    /// comma-separated aliases (`mov,mp4,m4a,3gp,3g2,mj2`, `matroska,webm`).
    pub(crate) fn from_format_name(name: &str) -> Self {
        if name
            .split(',')
            .any(|alias| matches!(alias, "mov" | "mp4" | "m4a" | "3gp" | "3g2" | "mj2"))
        {
            return Container::Mp4;
        }
        match name.split(',').next().unwrap_or(name) {
            "matroska" => Container::Matroska,
            "mp3" => Container::Mp3,
            // FFmpeg detects raw ADTS AAC as `aac`; the muxer that writes it is `adts`.
            "aac" => Container::Other("adts".to_string()),
            other => Container::Other(other.to_string()),
        }
    }

    pub(crate) fn of_input(input: &ff::format::context::Input) -> Self {
        Self::from_format_name(input.format().name())
    }

    pub(crate) fn of_output(output: &ff::format::context::Output) -> Self {
        Self::from_format_name(output.format().name())
    }

    pub(crate) fn from_extension(extension: &str) -> Option<Self> {
        crate::audio::format_for_extension(extension).map(|spec| spec.muxer.into())
    }

    pub(crate) fn is_mp4_family(&self) -> bool {
        *self == Container::Mp4
    }

    /// FLAC, whose muxer writes the generic `comment` key as the Vorbis
    /// `DESCRIPTION` field. Ogg files are not an ABB input format.
    pub(crate) fn uses_vorbis_comments(&self) -> bool {
        *self == Container::Other("flac".to_string())
    }

    /// Whether the container keeps each chapter's end. MP4 players end a
    /// chapter where the next starts, so only its starts and titles are kept.
    pub(crate) fn stores_chapter_ends(&self) -> bool {
        !self.is_mp4_family()
    }

    /// The FFmpeg key the container stores the album sort under: MP3 names
    /// the ID3 frame `album-sort`; the others `sort_album`.
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
            Container::Other(name) => name,
        }
    }
}

pub(crate) fn classify(path: &Path) -> Result<Container> {
    ff::init().map_err(AppError::Ffmpeg)?;

    let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
    let container = Container::from_format_name(ictx.format().name());
    log::info!(
        "metadata_route artifact={} input_format={} route={container:?}",
        crate::diagnostics::artifact_id(path),
        ictx.format().name()
    );
    Ok(container)
}

#[cfg(test)]
#[path = "container_tests.rs"]
mod tests;
