# Codex Feedback – New UI Plan Audit

## Scope
Audit of `docs/planning/newUI_plan.md` against current repo state to ensure the plan covers the work required to ship the new UI that mirrors `docs/specs/UI_mock/new_UI_final.html`.

## Findings

1. **Metadata contract incomplete for planned tag preview grid.**  
   *Current state:* `AudiobookMetadata` (Rust + TS) only includes basic fields (title, artist, album, composer, genre, date, track/disk, comment, description, cover art). `StatusPanel.getCurrentMetadata()` even stuffs `series` into `album` because there is no dedicated slot.  
   *Risk:* The new UI surfaces MVNM/MVIN/TSOA/etc., but nothing persists them, so the UI changes would be cosmetic.  
   *Plan update:* Expand the plan to add the new metadata fields end-to-end (Rust struct, serializer, Lofty reader/writer, TS types, DOM bindings) so the preview grid matches what is actually written to files.

2. **Cover-art optimization misses auto-loaded artwork.**  
   *Current state:* The plan optimizes images only in `load_cover_art_file`, yet most sessions auto-populate cover art via `read_audio_metadata` when files are imported. Those bytes (possibly large PNGs) feed straight back into `metadata.cover_art`.  
   *Risk:* Embedded art copied from source files would bypass the optimizer entirely, so the stated 500 KB/PNG guard would not hold.  
   *Plan update:* Extend the optimization step to cover both ingress paths—either optimize inside `read_audio_metadata` (before returning bytes) or re-run optimization right before `metadata::writer::write_cover_art` / finalize stage so every embedding is sanitized.

3. **Advanced encoder panel wiring undefined/outdated.**  
   *Current state:* The repo already split the encoder panel into `src/ui/encoderPanel/*`, and `StatusPanel` pulls encoder settings from `window.EncoderSettingsProvider()`, yet nothing registers that provider and the plan still references a single `encoderPanel.ts`.  
   *Risk:* Even after UI refresh, the backend would continue using fallback defaults; advanced controls would have no effect.  
   *Plan update:* Target the real module layout and add explicit steps to capture encoder form state, persist it (using `state.ts`), and expose it via `window.EncoderSettingsProvider` so `StatusPanel` sends those values to `process_audiobook_files_v2`.

## Recommendation
Revise `docs/planning/newUI_plan.md` to incorporate the three gaps above before implementation begins. This keeps the UI replacement scoped while ensuring the new surface area is actually powered by backend data.

