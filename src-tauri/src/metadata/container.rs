//! Container classification for metadata routing.

use std::path::Path;

use ffmpeg_next as ff;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContainerRoute {
    Mp4Family,
    Other { remux_output_format: String },
}

impl ContainerRoute {
    pub(crate) fn remux_output_format(&self) -> Option<&str> {
        match self {
            Self::Mp4Family => None,
            Self::Other {
                remux_output_format,
            } => Some(remux_output_format.as_str()),
        }
    }
}

pub(crate) fn classify(path: &Path) -> Result<ContainerRoute> {
    ff::init().map_err(AppError::Ffmpeg)?;

    let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
    Ok(classify_format_name(ictx.format().name()))
}

pub(crate) fn classify_format_name(format_name: &str) -> ContainerRoute {
    if is_mp4_family_format(format_name) {
        return ContainerRoute::Mp4Family;
    }

    ContainerRoute::Other {
        remux_output_format: remux_output_format_for(format_name).to_string(),
    }
}

fn primary_format_name(format_name: &str) -> &str {
    format_name.split(',').next().unwrap_or(format_name)
}

fn remux_output_format_for(format_name: &str) -> &str {
    match primary_format_name(format_name) {
        // FFmpeg detects raw ADTS AAC with demuxer name `aac`, but the writable
        // muxer name is `adts`.
        "aac" => "adts",
        format => format,
    }
}

fn is_mp4_family_format(format_name: &str) -> bool {
    format_name
        .split(',')
        .any(|name| matches!(name, "mov" | "mp4" | "m4a" | "3gp" | "3g2" | "mj2"))
}

#[cfg(test)]
mod tests {
    use super::{
        classify_format_name, is_mp4_family_format, primary_format_name, remux_output_format_for,
        ContainerRoute,
    };

    #[test]
    fn classifies_ffmpeg_mp4_family_format_name() {
        assert!(is_mp4_family_format("mov,mp4,m4a,3gp,3g2,mj2"));
    }

    #[test]
    fn keeps_non_mp4_format_name_for_forced_remux_output() {
        assert!(!is_mp4_family_format("mp3"));
        assert_eq!(primary_format_name("matroska,webm"), "matroska");
    }

    #[test]
    fn maps_demuxer_names_to_writable_remux_output_formats() {
        assert_eq!(remux_output_format_for("aac"), "adts");
        assert_eq!(remux_output_format_for("mp3"), "mp3");
        assert_eq!(remux_output_format_for("wav"), "wav");
        assert_eq!(remux_output_format_for("flac"), "flac");
    }

    #[test]
    fn classifies_format_name_without_reopening_path() {
        assert_eq!(
            classify_format_name("mov,mp4,m4a,3gp,3g2,mj2"),
            ContainerRoute::Mp4Family
        );
        assert_eq!(
            classify_format_name("mp3"),
            ContainerRoute::Other {
                remux_output_format: "mp3".to_string()
            }
        );
    }
}
