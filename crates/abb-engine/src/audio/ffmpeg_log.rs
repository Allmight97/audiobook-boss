use ffmpeg_next as ff;

/// FFmpeg's own log stays at Warning. At Info the MP4 demuxer prints an AAX
/// file's checksum, which public tables map to the account's activation bytes.
pub(crate) fn limit_ffmpeg_logging() {
    ff::util::log::set_level(ff::util::log::Level::Warning);
}
