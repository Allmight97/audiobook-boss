//! Container classification for metadata routing.

use std::path::Path;

use ffmpeg_next as ff;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContainerRoute {
    Mp4Family,
    Other { format_name: String },
}

impl ContainerRoute {
    pub(crate) fn detected_output_format(&self) -> Option<&str> {
        match self {
            Self::Mp4Family => None,
            Self::Other { format_name } => Some(format_name.as_str()),
        }
    }
}

pub(crate) fn classify(path: &Path) -> Result<ContainerRoute> {
    ff::init().map_err(AppError::Ffmpeg)?;

    let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
    let input_format = ictx.format();
    let format_name = input_format.name();
    if is_mp4_family_format(format_name) {
        return Ok(ContainerRoute::Mp4Family);
    }

    Ok(ContainerRoute::Other {
        format_name: primary_format_name(format_name).to_string(),
    })
}

fn primary_format_name(format_name: &str) -> &str {
    format_name.split(',').next().unwrap_or(format_name)
}

fn is_mp4_family_format(format_name: &str) -> bool {
    format_name
        .split(',')
        .any(|name| matches!(name, "mov" | "mp4" | "m4a" | "3gp" | "3g2" | "mj2"))
}

#[cfg(test)]
mod tests {
    use super::{is_mp4_family_format, primary_format_name};

    #[test]
    fn classifies_ffmpeg_mp4_family_format_name() {
        assert!(is_mp4_family_format("mov,mp4,m4a,3gp,3g2,mj2"));
    }

    #[test]
    fn keeps_non_mp4_format_name_for_forced_remux_output() {
        assert!(!is_mp4_family_format("mp3"));
        assert_eq!(primary_format_name("matroska,webm"), "matroska");
    }
}
