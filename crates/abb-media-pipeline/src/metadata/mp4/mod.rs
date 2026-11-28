pub(crate) mod boxes;
#[cfg(test)]
pub(crate) mod chpl;
pub(crate) mod ilst;
pub(crate) mod metadata;
pub(crate) mod writer;

pub(crate) use metadata::{
    looks_like_mp4_family_file, read_bounded_cover_art, read_metadata, write_metadata,
};

#[cfg(test)]
pub(crate) use ilst::{AtomData, AtomIdent};
#[cfg(test)]
pub(crate) use writer::{read_tag, write_chapters, write_tag};
