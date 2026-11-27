pub(crate) mod boxes;
pub(crate) mod chpl;
pub mod ilst;
pub(crate) mod metadata;
pub(crate) mod writer;

pub use ilst::{AtomData, AtomIdent, IlstItem, Mp4Tag};
pub use metadata::{
    looks_like_mp4_family_file, read_bounded_cover_art, read_metadata, write_metadata,
};
pub use writer::{read_tag, write_chapters, write_tag};
