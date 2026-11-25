pub fn limit_ffmpeg_logging() {
    abb_ffmpeg::set_log_level(abb_ffmpeg::LogLevel::Warning);
}
