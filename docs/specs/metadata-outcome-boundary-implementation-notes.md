# Metadata Outcome Boundary Implementation Notes

Date started: 2026-05-19
Branch: `roadmap/metadata-outcome-boundary`

## Decisions Not In The Roadmap

- MB1/MB2: Implemented the backend boundary in `intent_plan.rs` rather than a
  new file. The existing file already owned effective/naming resolution, so this
  kept the private cluster cohesive while changing the public strip.
- MB1/MB2: `NamingMetadata` is public and re-exported from the crate root
  because `output_artifact::build_output_path_preview` is a public test-facing
  API. Its fields remain private; callers build it through
  `NamingMetadata::from_metadata`.
- MB2: Kept `MetadataOutcomeRequest` to `input_path` plus `intent_patch`; no
  use-case enum was needed for the current processing/save/preview paths.

## Behavior Mutations

- No intentional user-visible behavior mutations. The Rust changes preserve
  current observable metadata, naming, and cover-art outcomes while moving
  ownership of projection and passthrough decisions into the metadata boundary.

## Tradeoffs

- MB2/MB3: Kept metadata writes as a sibling boundary function rather than a
  field on `MetadataOutcomePlan`. Processing should not carry write facts it
  will never consume, while metadata-only save still enters through the same
  metadata boundary via `plan_metadata_write`.
- MB3/MB5: `CoverArtPassthroughPolicy` is public/test-facing because
  `audio::process_audiobook_with_context` is public for integration tests. This
  keeps finalization policy typed end-to-end instead of converting back to a
  boolean at the processor edge.
- MB4: Kept the `previewOutputPath({ metadata })` tauriClient property name
  unchanged to avoid a TS runtime boundary contract change. Renamed local
  workflow concepts to `previewMetadataDraft`, `metadataIntentByPath`, and
  `buildOutputPathPreviewContext`.
- MB5: Moved passthrough cover-art merge behavior into
  `metadata::passthrough::merge_passthrough_cover_art` so native and external
  processors share one metadata-owned rule after the passthrough policy is
  applied.
- MB6-lite: Renamed active canon references from Metadata Intent Plan to
  Metadata Outcome Plan. Older historical specs were left unchanged.

## Test Changes

- Updated contract/unit tests to target `plan_metadata_outcome`,
  `plan_metadata_write`, `NamingMetadata`, and `CoverArtPassthroughPolicy`.
- Updated output-plan workflow tests statically for the frontend naming cleanup.
- Updated `scripts/check-public-api-strips.sh` so the metadata public-strip
  assertion captures full multiline `intent_plan` export blocks.

## Testing Gaps

- First focused Rust contract run exposed two warning-gate issues and one
  stale external-FDK unit-test helper name. Fixes: type-annotated
  `MetadataOutcomePlan` consumption in processing, kept metadata write coverage
  on `plan_metadata_write`, and updated the external-FDK test to call
  `merge_passthrough_cover_art`.
- Follow-up Rust compile exposed the remaining public processor test callsites
  that still passed a boolean passthrough flag. Updated them to pass
  `CoverArtPassthroughPolicy::Preserve` and removed the now-unused
  `allow_passthrough_cover_art` helper.
- No remaining known automated testing gaps for this roadmap slice. Validation
  covered metadata contracts, naming projection, save/clear intent, cover-art
  passthrough policy, processor finalization, frontend metadata intent
  workflow, public API strips, context surface, and the full standard gate.
  The existing xHE-AAC fixture test remains environment-gated and was ignored
  by the standard run because `ABB_XHE_AAC_FIXTURE` and external libfdk FFmpeg
  were not configured.

## PR Summary Notes

- Added `MetadataOutcomeRequest`, `MetadataOutcomePlan`, `NamingMetadata`,
  `CoverArtPassthroughPolicy`, `plan_metadata_outcome`, and
  `plan_metadata_write` under the metadata boundary.
- Refactored processing/output planning and native/external processor
  finalization to consume metadata-owned naming metadata and cover-art
  passthrough policy.
- Kept Tauri command payloads and generated TypeScript bindings unchanged; only
  local frontend naming was clarified around draft metadata and intent-by-path.
- Updated active Public API Strip docs/checks from Metadata Intent Plan to
  Metadata Outcome Plan.
- Validation completed:
  `cargo test -p audiobook-boss --lib contract_tests`,
  `cargo test -p audiobook-boss --lib metadata`,
  `cargo test -p audiobook-boss --lib processing`,
  `cargo test -p audiobook-boss --test unit_output_path_naming_tests`,
  `cargo test -p audiobook-boss --lib merge_passthrough_cover_art`,
  `cargo test -p audiobook-boss --test integration_metadata_tests metadata`,
  `cargo test -p audiobook-boss --test integration_processing_flow_tests metadata_preservation`,
  `cargo test -p audiobook-boss --test integration_processing_flow_tests preview_and_full_processing_preserve_series_metadata_from_source_file`,
  targeted Vitest workflow/runtime files, `scripts/check-public-api-strips.sh`,
  `scripts/check-no-bridge-imports.sh`, `bash scripts/check-context-surface.sh`,
  and logged `scripts/checks.sh standard` (`/tmp/abb-standard-checks.log`,
  exit status 0).

## Unresolved Follow-Ups

- None for MB1-MB5.
