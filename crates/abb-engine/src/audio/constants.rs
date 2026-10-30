pub const TEMP_MERGED_FILENAME: &str = "merged.m4b";

pub const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// Allow normal bitrate variation around the 64 kbps compact-audio target.
pub(super) const COMPACT_AUDIO_MAX_BITRATE: u32 = 72_000;
