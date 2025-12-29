//! FFmpeg-Next metadata integration bridge
//!
//! This module provides conversion and integration between our AudiobookMetadata
//! structures and ffmpeg-next metadata APIs, enabling direct metadata embedding
//! during the encoding process.

use crate::errors::Result;
use ffmpeg_next as ff;

pub use super::cover_art::{
    add_cover_art_stream_pre_header, detect_cover_art_format, write_cover_art_packet_post_header,
    CoverFormat,
};
pub use super::ffmpeg_dict::{set_container_metadata, validate_metadata_compatibility};
pub use super::remux::rewrite_metadata_with_ffmpeg;

#[allow(dead_code)]
pub fn metadata_to_ffmpeg_dict(metadata: &super::AudiobookMetadata) -> Result<ff::Dictionary<'_>> {
    super::ffmpeg_dict::metadata_to_ffmpeg_dict(metadata)
}
