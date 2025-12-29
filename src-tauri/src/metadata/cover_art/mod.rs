pub(crate) mod embedding;
pub(crate) mod format;

pub use embedding::{add_cover_art_stream_pre_header, write_cover_art_packet_post_header};
pub use format::{detect_cover_art_format, CoverFormat};
