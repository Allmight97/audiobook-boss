# Unified Encoder Plan — Single Source of Truth

Last updated: 2025-08-23

This document replaces the following and becomes the single source of truth:
- docs/planning/new_encoder/encoder_imp_plan.md
- docs/planning/encoder_implementation_plan_updated_aac_for_audiobooks_aug_2025.md

Guiding principles
- Solo-dev/prototype-first: hard cutovers, minimal legacy, small iterative PRs.
- ffmpeg-next in-process is the default engine. External FDK is optional, user-provided, and used only when explicitly enabled.
- Defaults: macOS → aac_at; non-macOS → native aac (AAC-LC).
- HE‑AAC v2 implies stereo. For mono, use HE‑AAC v1.

## Canonical API Types

TypeScript → Rust (serde) at the IPC boundary.

EncoderSettingsV2
- flavor: 'auto' | 'aac_at' | 'native_aac' | 'external_fdk'
- bitrateKbps: 56 | 64 | 72 | 80 | 88 | 96
- channels: 1 | 2
- profile?: 'lc' | 'he' | 'he_v2'             // hidden for native_aac
- vbr?: { enabled: boolean; level?: number }    // encoder-specific; for aac_at: level 0–14 (0 best)
- optimizeLcLowBitrate?: boolean                // native only; sets 32kHz at ≤64 kbps
- externalFfmpegPath?: string                   // absolute path to a user-provided ffmpeg (with libfdk_aac)

Resolver (Rust)
- resolve_encoder_config(settings_v2) → internal config
  - Validates inter-field constraints (e.g., HEv2 → channels=2)
  - Normalizes encoder-specific support (VBR, profile) and records ignored/unsupported flags for logging

Note: We hard-cut to EncoderSettingsV2; legacy v1 surfaces are removed.

---

## Phases (P1…Pn)

Each phase includes Outcome, Scope, Manual test suggestions, Success criteria.

### P1 — Contracts & Validation (done)
- Outcome: Type-safe EncoderSettingsV2 contracts; invalid combinations rejected.
- Scope
  - Add EncoderSettingsV2 (TS + Rust serde)
  - Backend validation: bitrate, HEv2 stereo-only, threads range, field-compatibility per flavor
  - Encoder-by-name helper (aac_at best-effort)
- Manual tests
  - HEv2 + mono → validation error; invalid bitrate/threads rejected
- Success
  - EncoderSettingsV2 accepted across IPC; invalid combos return user-facing errors

### P2 — Processing Command & Plumbing (done)
- Outcome: Single v2 command carries EncoderSettingsV2 through to the pipeline; v1 removed.
- Scope
  - Tauri command: process_audiobook_files_v2({ inputFiles, outputDir, settings })
  - Derive plan/context; remove old v1 command/types
- Manual tests
  - Smoke run (preview + full job) with v2 payload
- Success
  - Only v2 path present; builds/tests green

### P3 — ffmpeg-next Mapping (aac_at/native) + Optimize + Summary Logs (current)
- Outcome: Selected encoder/opts honored in-process; canonical INFO summary per job.
- Scope
  - Selection: macOS→aac_at (if present) else native; non-macOS→native
  - Profiles: set best-effort via av_opt_set_int for HE v1/v2; omit for native LC
  - AAC coder: native only → aac_coder=twoloop|fast
  - Native optimize: if optimizeLcLowBitrate && bitrate≤64k ⇒ set output sample rate=32000; else pass-through
  - Summary logs (INFO):
    - encoder=<aac_at|aac(native)|external-fdk>
    - profile=<lc|he|he_v2|none> bitrate=<kbps> ch=<1|2> sr=<Hz> vbr=<off|level> afterburner=<0|1> notes=[ignored:...,unsupported:...]
  - DEBUG logs per option set; attach ignored/unsupported notes from resolver
- Manual tests
  - macOS: aac_at 64k mono; HEv1 mono; HEv2 stereo; native optimize at 56k
- Success
  - INFO line present; options applied or logged as ignored without runtime failures

### P4 — Advanced UI (Encoder Panel)
- Outcome: Users can pick encoder flavor and advanced options; UX enforces constraints.
- Scope
  - Encoder flavor: Auto | Apple AAC (macOS) | Native AAC | External FFmpeg (FDK)
  - Bitrate: 56..96; Channels: 1|2 with HEv2→stereo enforcement
  - Profile: LC | HE (v1) | HE (v2) (hidden for native)
  - Advanced:
    - VBR toggle + level (where supported); aac_at quality 0–14; CBR remains default
    - Native optimize toggle: default ON if bitrate < 64k, OFF otherwise
    - External FDK: visible toggle and path selector; path required to enable
  - Disabled states + tooltips for unsupported combos
- Manual tests
  - HEv2 forces stereo; optimize toggle defaults per bitrate threshold; path selector for FDK is present
- Success
  - UI emits valid EncoderSettingsV2; invalid combos blocked in UI and still validated in backend

### P5 — External FDK Fallback (visible feature; optional use)
- Outcome: When user supplies a valid ffmpeg path with libfdk_aac and enables the toggle, jobs run through a sanitized external CLI; otherwise in-process is used.
- Scope
  - Detection UX: path selector (user-provided). Optional probe action can run `ffmpeg -hide_banner -v 0 -encoders`
  - CLI builder: sanitize paths/args; explicit flags (profile/bitrate/channels/rate/vbr/afterburner)
  - Invocation: only when flavor=external_fdk AND path validated; otherwise fallback to in-process with a warning
  - Logging: INFO summary includes notes=[external-fdk] when used; warning when falling back
- Manual tests
  - Bogus path → fallback with warning; valid path → external; INFO notes reflect mode
- Success
  - No external invocation without opt-in + valid path; progress/cancellation parity preserved

### P6 — Logging & Observability
- Outcome: Structured, minimal noise logs to aid QA and debugging.
- Scope
  - Enforce canonical INFO line per job
  - DEBUG per option set; record ignored/unsupported from resolver
- Manual tests
  - Review logs across scenarios; confirm INFO schema
- Success
  - CI/integration test asserts INFO-line presence/schema

### P7 — Tests & QA
- Outcome: Confidence across validation, mapping, UI constraints.
- Scope
  - Unit: validators (HEv2 stereo), resolver (ignored flags), mapping (profile/coder/threads/optimize)
  - Integration: E2E encode incl. summary logs on macOS; smoke native on non-macOS
  - Stability: preview/full; multiple input samples; artifact checks at 56/64/80 kbps
- Manual tests
  - Quality sanity at 56 vs 80 kbps; HEv2 stereo artifacts; summarize results
- Success
  - Tests stable; no regressions or dead code introduced

### P8 — Docs & Consolidation
- Outcome: One authoritative plan + concise external docs.
- Scope
  - Keep this as the planning “bible”; archive legacy plans
  - Update external-apis/ffmpeg-next.md (profiles/threads specifics; aac_at VBR notes)
  - Add short “Enable FDK” help doc (path selector + optional probe)
- Manual checks
  - Links resolve; help text matches actual UI
- Success
  - Single plan doc; redundant docs archived

---

## Manual Testing Cheat Sheet
- macOS aac_at 64k mono CBR; HEv1 mono @64k; HEv2 stereo @64k
- Native AAC 56k mono with optimize ON → sr=32000; 80k mono with optimize OFF → pass-through policy
- External FDK: invalid path (fallback+warn) vs valid path (external); confirm INFO notes

## References
- ffmpeg-next (Rust) encoder API (options, profiles)
- FFmpeg documentation (profiles/options/threads)
- TypeScript strict module patterns
- Tauri v2 command/event patterns
