use super::{AudiobookMetadata, MetadataIntentPatch};
use crate::errors::Result;
use std::path::Path;

pub(crate) fn resolve_effective_processing_metadata(
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

pub(crate) fn resolve_naming_metadata(
    resolved_metadata: Option<&AudiobookMetadata>,
    input_path: Option<&Path>,
    patch: Option<&MetadataIntentPatch>,
) -> Option<AudiobookMetadata> {
    let mut naming_metadata = resolved_metadata.cloned()?;

    if input_path.is_some() && patch.is_none() {
        scrub_legacy_source_series_parts_for_naming(&mut naming_metadata);
    }

    Some(naming_metadata)
}

fn scrub_legacy_source_series_parts_for_naming(metadata: &mut AudiobookMetadata) {
    scrub_invalid_series_part_for_naming(&mut metadata.series_part);
    scrub_invalid_series_part_for_naming(&mut metadata.subseries_part);
}

fn scrub_invalid_series_part_for_naming(value: &mut Option<String>) {
    let should_clear = value
        .as_deref()
        .map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
        .is_some_and(|trimmed| crate::metadata::validate_series_part(trimmed).is_err());

    if should_clear {
        *value = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_effective_processing_metadata, resolve_naming_metadata};
    use crate::metadata::{AudiobookMetadata, MetadataIntentPatch, PatchOp};
    use crate::output_artifact::{build_output_path, OutputNamingConfig};
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
        let outcome = resolve_effective_processing_metadata(Some(missing_source), None);
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

        let resolved =
            resolve_effective_processing_metadata(None, Some(&patch)).expect("metadata resolves");

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

        let err = resolve_effective_processing_metadata(None, Some(&patch))
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
        let output_path = build_output_path(
            Path::new("/tmp"),
            Some(&effective),
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

        let naming =
            resolve_naming_metadata(Some(&metadata), Some(Path::new("/tmp/source.m4b")), None)
                .expect("naming metadata should exist");

        assert_eq!(naming.title.as_deref(), Some("Legacy Source"));
        assert_eq!(naming.series.as_deref(), Some("Series"));
        assert_eq!(naming.series_part, None);
        assert_eq!(naming.subseries.as_deref(), Some("Subseries"));
        assert_eq!(naming.subseries_part, None);

        let output_path = build_output_path(
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
    fn naming_metadata_keeps_patch_validation_strict() {
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

        let naming = resolve_naming_metadata(
            Some(&metadata),
            Some(Path::new("/tmp/source.m4b")),
            Some(&patch),
        )
        .expect("naming metadata should exist");

        let err = build_output_path(
            Path::new("/tmp"),
            Some(&naming),
            OutputNamingConfig::default(),
            Some(Path::new("/tmp/source.m4b")),
        )
        .expect_err("patched legacy series part should remain a hard failure");

        assert!(
            err.to_string().contains("Series sequence"),
            "unexpected error: {err}"
        );
    }
}
