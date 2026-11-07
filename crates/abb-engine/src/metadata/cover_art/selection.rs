use crate::errors::{AppError, Result};

use super::super::passthrough::PassthroughMetadata;
use super::super::AudiobookMetadata;
use abb_metadata_core::PatchOp;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResolvedCover<'a> {
    Explicit(&'a Vec<u8>),
    Passthrough(&'a Vec<u8>),
}

impl<'a> ResolvedCover<'a> {
    pub(crate) fn select(
        cover: Option<&'a PatchOp<Vec<u8>>>,
        passthrough: Option<&'a PassthroughMetadata>,
    ) -> Option<Self> {
        match cover {
            Some(PatchOp::Set(bytes)) => Some(ResolvedCover::Explicit(bytes)),
            Some(PatchOp::Clear) => None,
            None => Self::from_passthrough(passthrough),
        }
    }

    /// The selection for a native encode, which takes the cover as part of the
    /// output's metadata: an empty cover there is a clear.
    pub(crate) fn select_for_encode(
        metadata: Option<&'a AudiobookMetadata>,
        passthrough: Option<&'a PassthroughMetadata>,
    ) -> Option<Self> {
        match metadata.and_then(|value| value.cover_art.as_ref()) {
            Some(bytes) if bytes.is_empty() => None,
            Some(bytes) => Some(ResolvedCover::Explicit(bytes)),
            None => Self::from_passthrough(passthrough),
        }
    }

    fn from_passthrough(passthrough: Option<&'a PassthroughMetadata>) -> Option<Self> {
        passthrough
            .and_then(|value| value.cover_art.as_ref())
            .filter(|bytes| !bytes.is_empty())
            .map(ResolvedCover::Passthrough)
    }

    pub(crate) fn source(self) -> &'static str {
        match self {
            Self::Explicit(_) => "explicit",
            Self::Passthrough(_) => "passthrough",
        }
    }

    pub(crate) fn bytes(self) -> &'a Vec<u8> {
        match self {
            Self::Explicit(bytes) | Self::Passthrough(bytes) => bytes,
        }
    }

    fn is_passthrough(self) -> bool {
        matches!(self, Self::Passthrough(_))
    }

    pub(crate) fn handle_embed_failure(self, error: AppError, stage: &str) -> Result<()> {
        if self.is_passthrough() {
            log::warn!("metadata_cover stage={stage:?} source=passthrough decision=skip reason=embed_failed error={error}");
            Ok(())
        } else {
            log::error!("metadata_cover stage={stage:?} source=explicit decision=fail reason=embed_failed error={error}");
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
