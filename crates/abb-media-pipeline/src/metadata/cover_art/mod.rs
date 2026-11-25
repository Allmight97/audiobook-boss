pub(crate) mod embedding;
pub(crate) mod format;
pub(crate) mod selection;

pub use embedding::{add_cover_stream, write_cover_packet};
pub use format::detect_cover_art_format;
pub use selection::ResolvedCover;
