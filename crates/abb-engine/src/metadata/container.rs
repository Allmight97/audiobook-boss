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
            Self::Mp4Family => Some("mp4"),
            Self::Other {
                remux_output_format,
            } => Some(remux_output_format.as_str()),
        }
    }
}

pub(crate) fn classify(path: &Path) -> Result<ContainerRoute> {
    ff::init().map_err(AppError::Ffmpeg)?;

    let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
    let route = classify_format_name(ictx.format().name());
    log::info!(
        "metadata_route artifact={} input_format={} route={route:?}",
        crate::diagnostics::artifact_id(path),
        ictx.format().name()
    );
    Ok(route)
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
#[path = "container_tests.rs"]
mod tests;
