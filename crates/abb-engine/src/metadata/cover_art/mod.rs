pub(crate) mod embedding;
pub(crate) mod format;
pub(crate) mod selection;

pub(crate) use embedding::{add_cover_stream, write_cover_packet};
pub use format::detect_cover_art_format;
pub(crate) use selection::ResolvedCover;
