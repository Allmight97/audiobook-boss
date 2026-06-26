//! Remux-time cover-art selection and embed-failure severity.
//!
//! Owns two decisions in one place: which cover wins when both an explicit
//! (metadata) and a passthrough (source-preserved) cover are available, and how
//! an embed failure is graded by that source. Explicit covers are mandatory —
//! a failed embed is fatal; passthrough covers are best-effort — a failed embed
//! is warned and skipped.

use crate::errors::{AppError, Result};

use super::super::passthrough::PassthroughMetadata;
use super::super::AudiobookMetadata;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResolvedCover<'a> {
    Explicit(&'a Vec<u8>),
    Passthrough(&'a Vec<u8>),
}

impl<'a> ResolvedCover<'a> {
    /// Select the cover to write: an explicit metadata cover wins over a
    /// passthrough cover, and an empty cover is dropped (`set | clear | noop`).
    pub(crate) fn select(
        metadata: Option<&'a AudiobookMetadata>,
        passthrough: Option<&'a PassthroughMetadata>,
    ) -> Option<Self> {
        metadata
            .and_then(|value| value.cover_art.as_ref())
            .map(ResolvedCover::Explicit)
            .or_else(|| {
                passthrough
                    .and_then(|value| value.cover_art.as_ref())
                    .map(ResolvedCover::Passthrough)
            })
            .filter(|selection| !selection.bytes().is_empty())
    }

    pub(crate) fn bytes(self) -> &'a Vec<u8> {
        match self {
            Self::Explicit(bytes) | Self::Passthrough(bytes) => bytes,
        }
    }

    fn is_passthrough(self) -> bool {
        matches!(self, Self::Passthrough(_))
    }

    /// Grade an embed-stage failure by cover source: passthrough covers warn and
    /// continue (`Ok`), explicit covers propagate the error (`Err`). This is the
    /// single owner of the severity decision shared by the pre-header stream and
    /// post-header packet remux stages.
    pub(crate) fn handle_embed_failure(self, error: AppError, stage: &str) -> Result<()> {
        if self.is_passthrough() {
            log::warn!("Could not preserve passthrough {stage} during metadata remux: {error}");
            Ok(())
        } else {
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_with_cover(bytes: Vec<u8>) -> AudiobookMetadata {
        AudiobookMetadata {
            cover_art: Some(bytes),
            ..Default::default()
        }
    }

    fn passthrough_with_cover(bytes: Vec<u8>) -> PassthroughMetadata {
        PassthroughMetadata {
            cover_art: Some(bytes),
            ..Default::default()
        }
    }

    #[test]
    fn explicit_cover_wins_over_passthrough() {
        let explicit = metadata_with_cover(vec![1, 2, 3]);
        let passthrough = passthrough_with_cover(vec![9, 9, 9]);
        let selection = ResolvedCover::select(Some(&explicit), Some(&passthrough))
            .expect("a cover is selected");
        assert!(matches!(selection, ResolvedCover::Explicit(_)));
        assert_eq!(selection.bytes(), &vec![1, 2, 3]);
    }

    #[test]
    fn passthrough_used_when_no_explicit_cover() {
        let passthrough = passthrough_with_cover(vec![9, 9, 9]);
        let selection =
            ResolvedCover::select(None, Some(&passthrough)).expect("a cover is selected");
        assert!(matches!(selection, ResolvedCover::Passthrough(_)));
        assert_eq!(selection.bytes(), &vec![9, 9, 9]);
    }

    #[test]
    fn empty_cover_is_dropped() {
        let empty_explicit = metadata_with_cover(Vec::new());
        let passthrough = passthrough_with_cover(vec![9, 9, 9]);
        // An empty explicit cover drops rather than falling back to passthrough:
        // an explicit empty cover is a clear, not a request for the source art.
        assert!(ResolvedCover::select(Some(&empty_explicit), Some(&passthrough)).is_none());
        assert!(ResolvedCover::select(None, None).is_none());
    }

    #[test]
    fn passthrough_embed_failure_warns_and_continues() {
        let passthrough = passthrough_with_cover(vec![9, 9, 9]);
        let selection = ResolvedCover::select(None, Some(&passthrough)).expect("selected");
        let result =
            selection.handle_embed_failure(AppError::General("boom".to_string()), "cover art");
        assert!(result.is_ok());
    }

    #[test]
    fn explicit_embed_failure_propagates() {
        let explicit = metadata_with_cover(vec![1, 2, 3]);
        let selection = ResolvedCover::select(Some(&explicit), None).expect("selected");
        let result =
            selection.handle_embed_failure(AppError::General("boom".to_string()), "cover art");
        assert!(result.is_err());
    }
}
