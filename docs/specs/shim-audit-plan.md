# Shim Audit Plan — March 2026

Audit of expired, dead, and incorrect fallback/shim code identified during an agent review session on 2026-03-16. Five items total. Three are clean deletes or safe fixes executable immediately. Two require owner/Codex input before execution.

Source: `docs/fallbacks.md` audit notes.

---

## Items

### Item 1 — FB-006: Delete dead `early_stop` branch in `engine.rs`

**File:** `src-tauri/src/audio/processor/engine.rs:242–251`

**Finding:** Unreachable code. The `early_stop` flag is set and consumed inside `frame_pipeline.rs` before `process_input_file` returns. When `engine.rs` receives `PreviewAction::Continue`, `early_stop` cannot be `true` — the pipeline already handled it and broke out of the packet loop below. The `if *ctx.early_stop { break; }` block under `PreviewAction::Continue` in `engine.rs` is structurally dead.

**Action:** Delete the `// FALLBACK[FB-006]` comment block and the `if *ctx.early_stop { ... break; }` inner check (~8 lines). The `early_stop` field on `FramePipelineCtx` stays — it is still correctly written and read inside `frame_pipeline.rs`. Remove FB-006 from `docs/fallbacks.md`.

**Risk:** None. Pure deletion of unreachable branch.

**Verification:** `scripts/checks.sh standard`. No behavior change possible.

**Questions needed:** None. Ready to execute.

**Codex response:** Agree. This is a clean delete. The `PreviewAction::Continue` branch in `engine.rs` still carries a legacy fallback check, but the actual early-stop behavior is already owned inside the frame pipeline. Execute as written: delete the dead branch and remove FB-006 from the register.

---

### Item 2 — FB-005: Delete `ABB_DISABLE_TWOOLOOP` typo alias

**File:** `src-tauri/src/audio/processor/encoder/options/native.rs:32–36` and inline test

**Finding:** The `ABB_DISABLE_TWOOLOOP` (double-O) env var was a typo introduced by Codex. This is a solo-dev project with no external users who could have this set in any env, CI, or `.env` file. The correct spelling `ABB_DISABLE_TWOLOOP` is the canonical name and already fully covered by the remaining test cases.

**Action:**
- Delete the `|| std::env::var("ABB_DISABLE_TWOOLOOP")...` block (3 lines + comment) from `build_native_options`.
- Delete the `"Backward compatibility: old typo still disables"` subtest block from `respects_twoloop_flag_and_env_override` (the `set_var("ABB_DISABLE_TWOOLOOP")` / `remove_var` block, ~6 lines).
- Remove FB-005 from `docs/fallbacks.md`.

**Risk:** None. Solo dev, no external consumers.

**Verification:** `scripts/checks.sh standard`. Remaining test still covers correct-spelling env override.

**Questions needed:** None. Ready to execute.

**Codex response:** Agree. Treat `ABB_DISABLE_TWOOLOOP` as dead internal shim residue, not a compatibility surface. Delete the typo alias, trim the test to the canonical env var only, and remove FB-005 from the register.

---

### Item 3 — FB-002: Fix wrong dimension fallback in cover art embedding

**File:** `src-tauri/src/metadata/cover_art/embedding.rs:130–141`

**Finding:** Bug. When `detect_image_dimensions` returns `None`, the code falls back to `(600, 600)` and proceeds to configure the codec context with those dimensions. If the actual image is any other size, the stream parameters are wrong — some muxers and players will reject the packet or silently corrupt the output. Skipping embedding is strictly better than embedding with fabricated parameters.

**Action:** In `configure_cover_art_stream_parameters`, replace the `None` arm:
```rust
// BEFORE
None => {
    log::warn!("FALLBACK[FB-002] defaulting cover art dimensions to 600x600 ...");
    (600, 600)
}
```
with a proper early return error:
```rust
// AFTER
None => {
    return Err(AppError::General(format!(
        "Could not detect dimensions for {:?} cover art ({} bytes); skipping embedding",
        format,
        cover_data.len()
    )));
}
```
The caller `add_cover_art_stream_pre_header` already handles `Err` from `configure_cover_art_stream_parameters` by logging a warn and returning `None` (skip). No caller changes needed.

Remove FB-002 from `docs/fallbacks.md`.

**Risk:** Low. Changes failure mode from "embed with wrong dimensions" to "skip embedding." Skip is the correct behavior and what the log message already implies to the user.

**Verification:** Confirm `integration_cover_art_compat_tests.rs` covers the skip path. If not, add a unit test: pass a 3-byte garbage buffer to `add_cover_art_stream_pre_header`, assert it returns `None`.

**Questions needed:** None. Ready to execute.

**Codex response:** Agree with the proposed direction. Returning an error from `configure_cover_art_stream_parameters` is the right fix because the caller already treats that path as "skip embedding and log." During execution, add or confirm a focused regression test for garbage cover-art bytes returning `None` from the pre-header helper.

---

### Item 4 — `normalizeProcessResult`: Remove dead inference fallbacks in `normalizers.ts`

**File:** `src/lib/tauri/normalizers.ts:166–230`

**Finding:** Two dead fallbacks inside `normalizeProcessResult`:

1. **`fallbackSummary`**: A `{ total, succeeded, failed }` object computed from `results` in case `normalized.summary` is missing. The Rust backend always emits `summary` (it is a non-optional field in the specta-generated contract). This code never fires.

2. **`jobType` inference**: Falls back to `results.length > 1 ? 'batch' : 'merge'` if `normalized.jobType` is absent. Same issue — always present per contract. Additionally, the inference is *wrong*: a merge job with multiple input parts (the normal use case) would incorrectly infer `'batch'`.

**Action:**
- Remove `fallbackSummary` computation. Use `normalized.summary` directly with a narrow cast.
- Remove the `results.length > 1` ternary. Use `normalized.jobType` directly.
- If the function signature `result: unknown` was chosen defensively rather than for a real reason (see question below), consider tightening the input type to match the generated contract, which would let TypeScript eliminate most of the remaining `isPlainRecord` casting.

**Risk:** Low. The contract is specta-generated and both fields are always present. If somehow absent (impossible under the current backend), the function now returns an honest `undefined`/bad value rather than a silently wrong inferred one — a more debuggable failure.

**Verification:** `scripts/checks.sh standard` + any existing tests covering `processAudiobookFiles` result handling.

**❓ Question for Codex / owner (required before executing):**

> `normalizeProcessResult` accepts `result: unknown` rather than the generated `ProcessCommandResult` type. Is this intentional? Specifically: is there any call site that passes a value originating outside the specta-generated `commands.processAudiobookFiles(...)` invocation path? Or was `unknown` used because Codex was not confident the contract was stable at the time? If the latter, we should change the signature to accept the generated type and drop the remaining defensive casting in one pass.

**Codex response:** I do not see an intentional reason for `unknown` here. The only live call path I found is the Tauri client wrapping the specta-generated `processAudiobookFiles(...)` invocation and then applying `normalizeProcessResult`; I did not find any caller feeding arbitrary external data into this function. This reads as defensive residue, not a real boundary requirement. I would execute this as a typed cleanup: tighten the parameter to the generated process-result type, remove the dead `summary` and `jobType` inference fallbacks, and keep only the nullish normalization that still serves the TS boundary elsewhere.

---

### Item 5 — FB-001: Narrow the ffmpeg re-read trigger in `reader.rs`

**File:** `src-tauri/src/metadata/reader.rs:22–75`

**Finding:** When mp4ameta successfully reads an M4B but any of `series`, `series_part`, `subseries`, `subseries_part`, or `cover_art` are `None`, the code opens the file a second time with ffmpeg to attempt backfilling. For most real audiobooks without subseries data, this means every read triggers two full container opens. The performance cost is real; the benefit for missing `subseries`/`subseries_part` specifically is negligible, because:
- Subseries data is rare in the wild.
- mp4ameta and ffmpeg read from the same underlying freeform atoms for this field.
- If mp4ameta didn't find it, ffmpeg almost certainly won't either.

**Action (proposed, pending answer below):** Narrow the re-read trigger condition from:
```rust
if metadata.series.is_none()
    || metadata.series_part.is_none()
    || metadata.subseries.is_none()
    || metadata.subseries_part.is_none()
    || metadata.cover_art.is_none()
```
to:
```rust
if metadata.cover_art.is_none()
    || (metadata.series.is_none() && metadata.series_part.is_none())
```
This preserves the meaningful fallback cases (cover art missing entirely; no series data at all from mp4ameta) while eliminating re-reads triggered solely by absent subseries fields.

FB-001 stays in the register but with an updated trigger description and pushed sunset.

**Risk:** Medium. Touches the read path for all M4B files. Needs a test confirming the narrowed condition still catches the cases it's meant to catch.

**Verification:** Existing metadata compatibility tests. May need a targeted test: an M4B with series+series_part in mp4ameta atoms but cover art stored only as an attached stream (the cover-art-only re-read case).

**❓ Question for Codex / owner (required before executing):**

> In your real-world test files or the files users typically bring to ABB, have you ever observed a case where:
> - mp4ameta reads `series` successfully, AND
> - ffmpeg backfills `subseries` or `subseries_part` that mp4ameta missed?
>
> If this has never been observed (or is theoretically impossible given how both libraries read the same `----:com.apple.iTunes:SERIES` freeform atom), then the `subseries`/`subseries_part` conditions can be dropped entirely from the trigger, not just combined. The answer determines whether the narrowed condition above is right, or whether an even simpler condition (`cover_art.is_none() || series.is_none()`) is correct.

**Codex response:** I do not have evidence of real files where mp4ameta reads primary `series` successfully and ffmpeg then uniquely backfills only `subseries` or `subseries_part`; both paths derive those secondary fields by splitting the same underlying series atoms, so I would drop `subseries`/`subseries_part` as standalone reread triggers. But the proposed narrowed condition here is still too aggressive: keep rereads when either primary `series` or primary `series_part` is missing, because the ffmpeg path still has legacy read keys that can backfill those primary fields. My recommended trigger is `metadata.cover_art.is_none() || metadata.series.is_none() || metadata.series_part.is_none()`. Keep FB-001, but update its trigger text to match that narrower, primary-field-only rule.

---

## Execution Order

| Order | Item | Blocked? |
|---|---|---|
| 1 | FB-006 — delete dead `early_stop` branch | No |
| 2 | FB-005 — delete `TWOOLOOP` typo alias | No |
| 3 | FB-002 — fix dimension fallback bug | No |
| 4 | `normalizeProcessResult` dead fallbacks | **Yes — needs Item 4 question answered** |
| 5 | FB-001 — narrow ffmpeg re-read trigger | **Yes — needs Item 5 question answered** |

Items 1–3 can execute as soon as approved. Items 4–5 are parked pending Codex/owner responses above.
