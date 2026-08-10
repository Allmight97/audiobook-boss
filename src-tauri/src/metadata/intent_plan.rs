use super::{AudiobookMetadata, MetadataIntentPatch, MetadataWritePlan, NamingMetadata};
use crate::errors::Result;
use crate::metadata::PassthroughMetadata;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverArtPassthroughPolicy {
    Preserve,
    SuppressAfterExplicitClear,
}

impl CoverArtPassthroughPolicy {
    pub(crate) fn from_intent_patch(patch: Option<&MetadataIntentPatch>) -> Self {
        if patch.is_some_and(MetadataIntentPatch::clears_cover_art) {
            Self::SuppressAfterExplicitClear
        } else {
            Self::Preserve
        }
    }

    pub(crate) fn apply_to_passthrough(
        self,
        passthrough: Option<PassthroughMetadata>,
    ) -> Option<PassthroughMetadata> {
        match self {
            Self::Preserve => passthrough,
            Self::SuppressAfterExplicitClear => {
                log::info!("cover_art_plan decision=skip_passthrough reason=explicit_cover_clear");
                passthrough.and_then(PassthroughMetadata::without_cover_art)
            }
        }
    }
}

pub(crate) struct MetadataOutcomeRequest<'a> {
    pub(crate) input_path: Option<&'a Path>,
    pub(crate) intent_patch: Option<&'a MetadataIntentPatch>,
}

#[derive(Debug, Clone)]
pub(crate) struct MetadataOutcomePlan {
    pub(crate) effective_metadata: Option<AudiobookMetadata>,
    pub(crate) naming_metadata: Option<NamingMetadata>,
    pub(crate) cover_art_passthrough: CoverArtPassthroughPolicy,
}

pub(crate) fn plan_metadata_outcome(
    request: MetadataOutcomeRequest<'_>,
) -> Result<MetadataOutcomePlan> {
    let effective_metadata =
        resolve_effective_processing_metadata(request.input_path, request.intent_patch)?;
    let naming_metadata = resolve_naming_metadata(
        effective_metadata.as_ref(),
        request.input_path,
        request.intent_patch,
    );
    Ok(MetadataOutcomePlan {
        effective_metadata,
        naming_metadata,
        cover_art_passthrough: CoverArtPassthroughPolicy::from_intent_patch(request.intent_patch),
    })
}

pub(crate) fn plan_metadata_write_for_path(
    input_path: &Path,
    patch: &MetadataIntentPatch,
) -> Result<MetadataWritePlan> {
    let source_metadata = crate::metadata::read_metadata(input_path)?;
    Ok(patch.to_write_plan_with_source(source_metadata)?)
}

fn resolve_effective_processing_metadata(
    input_path: Option<&Path>,
    patch: Option<&MetadataIntentPatch>,
) -> Result<Option<AudiobookMetadata>> {
    match (input_path, patch) {
        (Some(path), Some(patch)) => {
            let source_metadata = crate::metadata::read_metadata(path)?;
            Ok(Some(patch.apply_to_metadata(source_metadata)?))
        }
        (Some(path), None) => Ok(Some(crate::metadata::read_metadata(path)?)),
        (None, Some(patch)) => Ok(Some(patch.to_processing_overlay()?)),
        (None, None) => Ok(None),
    }
}

fn resolve_naming_metadata(
    resolved_metadata: Option<&AudiobookMetadata>,
    input_path: Option<&Path>,
    patch: Option<&MetadataIntentPatch>,
) -> Option<NamingMetadata> {
    let mut naming_metadata = resolved_metadata.map(NamingMetadata::from_metadata)?;

    let series_family_touched = patch.is_some_and(MetadataIntentPatch::touches_series_family);
    if input_path.is_some() && !series_family_touched {
        naming_metadata.scrub_legacy_source_series_parts_for_naming();
    }

    Some(naming_metadata)
}

#[cfg(test)]
mod tests {
    use super::{plan_metadata_outcome, MetadataOutcomeRequest};
    use crate::metadata::{
        AudiobookMetadata, CoverArtPassthroughPolicy, MetadataIntentPatch, PatchOp,
    };
    use crate::output_artifact::naming::build_output_path_preview;
    use crate::output_artifact::OutputNamingConfig;
    use std::path::Path;

    fn sample_source_metadata() -> AudiobookMetadata {
        AudiobookMetadata {
            title: Some("A Change of Plans".to_string()),
            artist: Some("Dennis E. Taylor".to_string()),
            album: Some("A Change of Plans".to_string()),
            series: Some("Checking".to_string()),
            ..AudiobookMetadata::new()
        }
    }

    #[test]
    fn effective_processing_metadata_no_patch_reads_source_or_errors() {
        let missing_source = Path::new("/path/that/does/not/exist/input.m4b");
        let outcome = plan_metadata_outcome(MetadataOutcomeRequest {
            input_path: Some(missing_source),
            intent_patch: None,
        });
        assert!(
            outcome.is_err(),
            "missing input should fail read, not return empty metadata"
        );
    }

    #[test]
    fn effective_processing_metadata_partial_set_and_clear_patch() {
        let patch = MetadataIntentPatch {
            series: PatchOp::Set("once again".to_string()),
            artist: PatchOp::Clear,
            ..Default::default()
        };

        let merged = patch
            .apply_to_metadata(sample_source_metadata())
            .expect("patch applies");

        assert_eq!(merged.artist, None);
        assert_eq!(merged.title.as_deref(), Some("A Change of Plans"));
        assert_eq!(merged.series.as_deref(), Some("once again"));
    }

    #[test]
    fn effective_processing_metadata_uses_overlay_without_source_file() {
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Overlay Only".to_string()),
            ..Default::default()
        };

        let resolved = plan_metadata_outcome(MetadataOutcomeRequest {
            input_path: None,
            intent_patch: Some(&patch),
        })
        .expect("metadata resolves")
        .effective_metadata;

        assert_eq!(
            resolved.and_then(|value| value.title),
            Some("Overlay Only".to_string())
        );
    }

    #[test]
    fn effective_processing_metadata_rejects_invalid_patch_values() {
        let patch = MetadataIntentPatch {
            date: PatchOp::Set("2024-99".to_string()),
            ..Default::default()
        };

        let err = plan_metadata_outcome(MetadataOutcomeRequest {
            input_path: None,
            intent_patch: Some(&patch),
        })
        .expect_err("invalid patch should fail");
        assert!(
            err.to_string().contains("Publication date"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolved_metadata_drives_naming_and_encoding_metadata_coherently() {
        let base = sample_source_metadata();
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Renamed Title".to_string()),
            ..Default::default()
        };
        let effective = patch
            .apply_to_metadata(base)
            .expect("metadata should resolve");
        let output_path = build_output_path_preview(
            Path::new("/tmp"),
            Some(&crate::metadata::NamingMetadata::from_metadata(&effective)),
            OutputNamingConfig::default(),
            None,
        )
        .expect("output path should build");

        assert!(
            output_path.to_string_lossy().contains("Renamed Title"),
            "output naming should use resolved metadata"
        );
        assert_eq!(effective.title.as_deref(), Some("Renamed Title"));
    }

    #[test]
    fn naming_metadata_scrubs_legacy_series_parts_for_untouched_source() {
        let metadata = AudiobookMetadata {
            title: Some("Legacy Source".to_string()),
            series: Some("Series".to_string()),
            series_part: Some("7/8".to_string()),
            subseries: Some("Subseries".to_string()),
            subseries_part: Some("2/3".to_string()),
            ..Default::default()
        };

        let naming = plan_metadata_outcome(MetadataOutcomeRequest {
            input_path: None,
            intent_patch: None,
        })
        .expect("empty request resolves");
        assert!(
            naming.naming_metadata.is_none(),
            "empty request should not invent naming metadata"
        );
        let naming = super::resolve_naming_metadata(
            Some(&metadata),
            Some(Path::new("/tmp/source.m4b")),
            None,
        )
        .expect("naming metadata should exist");

        assert_eq!(naming.title(), Some("Legacy Source"));
        assert_eq!(naming.series(), Some("Series"));
        assert_eq!(naming.series_part(), None);
        assert_eq!(naming.subseries(), Some("Subseries"));
        assert_eq!(naming.subseries_part(), None);

        let output_path = build_output_path_preview(
            Path::new("/tmp"),
            Some(&naming),
            OutputNamingConfig::default(),
            Some(Path::new("/tmp/source.m4b")),
        )
        .expect("legacy source naming should no longer fail");

        assert!(
            output_path.to_string_lossy().contains("Legacy Source"),
            "output naming should still use source metadata title"
        );
    }

    #[test]
    fn naming_metadata_scrubs_legacy_series_parts_for_non_series_patch() {
        let metadata = AudiobookMetadata {
            title: Some("Patched Source".to_string()),
            series: Some("Series".to_string()),
            series_part: Some("7/8".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Renamed".to_string()),
            ..Default::default()
        };

        let naming = super::resolve_naming_metadata(
            Some(&metadata),
            Some(Path::new("/tmp/source.m4b")),
            Some(&patch),
        )
        .expect("naming metadata should exist");

        assert_eq!(naming.title(), Some("Patched Source"));
        assert_eq!(naming.series_part(), None);

        let output_path = build_output_path_preview(
            Path::new("/tmp"),
            Some(&naming),
            OutputNamingConfig::default(),
            Some(Path::new("/tmp/source.m4b")),
        )
        .expect("non-series patch should not fail on inherited legacy part");

        assert!(
            output_path.to_string_lossy().contains("Patched Source"),
            "output naming should still use source metadata"
        );
    }

    #[test]
    fn naming_metadata_keeps_series_patch_validation_strict() {
        let metadata = AudiobookMetadata {
            title: Some("Patched Source".to_string()),
            series: Some("Series".to_string()),
            series_part: Some("7/8".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch {
            series: PatchOp::Set("Renamed Series".to_string()),
            ..Default::default()
        };

        let naming = super::resolve_naming_metadata(
            Some(&metadata),
            Some(Path::new("/tmp/source.m4b")),
            Some(&patch),
        )
        .expect("naming metadata should exist");

        let err = build_output_path_preview(
            Path::new("/tmp"),
            Some(&naming),
            OutputNamingConfig::default(),
            Some(Path::new("/tmp/source.m4b")),
        )
        .expect_err("series patch should keep inherited invalid part visible");

        assert!(
            err.to_string().contains("Series sequence"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn outcome_plan_reports_cover_art_clear_policy() {
        let patch = MetadataIntentPatch {
            cover_art: PatchOp::Clear,
            ..Default::default()
        };

        let outcome = plan_metadata_outcome(MetadataOutcomeRequest {
            input_path: None,
            intent_patch: Some(&patch),
        })
        .expect("metadata outcome resolves");

        assert!(
            matches!(
                outcome.cover_art_passthrough,
                CoverArtPassthroughPolicy::SuppressAfterExplicitClear
            ),
            "explicit cover clear must suppress source cover art passthrough"
        );
    }
}
