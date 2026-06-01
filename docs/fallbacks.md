# Fallback Register

Active register for fallback behavior that is still enforced by repo checks.
Keep entries here only when they materially protect product behavior, output integrity, real external-file interoperability, or the main release-quality check.
Best-effort UI preference persistence and tooling convenience fallbacks should be trimmed out instead of living here indefinitely.
Repo checks validate register sunsets, source-adjacent marker sunsets, and any renewal dates as real calendar dates.
Boundary smells, design smells, and implementation smells belong in active specs, Architecture Scout reports, or targeted issues unless they are actual fallback/shim behavior with enforcement markers and a removal path.

| ID | Location | Trigger | Observe | Sunset | Issue | Audit Status |
| --- | --- | --- | --- | --- | --- | --- |
| FB-019 | `src-tauri/src/commands/metadata_lookup/service.rs` | Failed Audnexus ASIN detail lookup may continue as text search | `MetadataLookupDiagnosticKind::AsinDirectLookupFallbackToTextSearch` in lookup response diagnostics | 2026-08-31 | #338 | RETAIN FOR NOW — preserves useful lookup when provider detail endpoint fails |
| FB-020 | `src-tauri/src/commands/metadata_lookup/service.rs` | One selected metadata source may fail while ABB returns available results from another selected source | `MetadataLookupDiagnosticKind::SourceFailedPartialResults` in lookup response diagnostics | 2026-08-31 | #338 | RETAIN FOR NOW — avoids discarding valid partial provider results |
| FB-021 | `src-tauri/src/commands/metadata_lookup/service.rs` | Failed Audnexus detail enrichment may return an Audible-only degraded result | `audible_only: true` result marker plus `MetadataLookupDiagnosticKind::AudnexusDetailFallbackToAudibleOnly` diagnostics | 2026-08-31 | #338 | RETAIN FOR NOW — preserves useful Audible search hits while marking degraded provenance |

Renewals, when needed, stay compact: append `renewal=YYYY-MM-DD; reason=...` to the Audit Status cell and make sure the renewal date is a valid calendar date that extends the sunset.
