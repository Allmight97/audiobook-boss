## Recommendations and Findings (ffmpeg-next, Lofty, Tauri)

This document consolidates concrete recommendations (grounded in official docs and this repo) and lists lingering shell-FFmpeg artifacts for cleanup.

### Decisions pending

2) Atomic write for metadata (Lofty)
- Option A (temp file swap; recommended if disk allows):
  - Copy original → temp; apply Lofty writes to temp; fsync; atomic rename over original.
  - Pros: minimizes corruption risk; easy rollback by keeping original until rename.
  - Cons: peak disk usage ≈ 2x.
- Option B (backup + in-place):
  - Copy original → backup; write in place; on failure, restore from backup.
  - Pros: lower peak disk usage.
  - Cons: brief risk window during in-place write.
Suggested default: A for M4B outputs, B only when disk constraints are tight.

3) Fast-path enablement
- Current evidence: with fast-path OFF, processing succeeds; ON exposed issues likely tied to encoder frame contract and sample sanitation.
- Safeguards now in place: centralized accumulator sanitation and debug-time frame validator.
- Recommendation: keep fast-path OFF by default until: (a) contract tests pass (PTS monotonicity, nb_samples ≤ frame_size when >0, format/layout/rate match), (b) long-run soak on representative inputs.

### Top improvements (beyond docs added today)

- Implement Lofty atomic write (Option A) in finalize stage for both tag and cover writes.
- Add debug-only `beforeunload` listener cleanup in the frontend to belt-and-suspenders event hygiene.
- Add contract tests (unit/integration) for frame sizing/PTS and a fuzz test injecting NaN/Inf samples pre-accumulator.
- Gate hot-loop logs with `log::log_enabled!`.

### External references

- FFmpeg-next (Rust): docs.rs
- FFmpeg AAC encoder options (aac_coder): ffmpeg.org/ffmpeg-codecs.html#aac-1
- Lofty: docs.rs/lofty
- Tauri v2 API: @tauri-apps/api

### Lingering shell-FFmpeg artifacts (for cleanup)

Criteria rubric: remove items that (1) mislead about current architecture, (2) add dead code paths, or (3) retain CLI-specific constants.

- `src-tauri/src/audio/constants.rs`: `FFMPEG_CONCAT_FORMAT`, `FFMPEG_CONCAT_SAFE_MODE`, `FFMPEG_PROGRESS_PIPE`
  - Impact: Low (constants unused by engine) but misleading; remove or annotate as legacy-only.
- `docs/reports/batch_final.md`: mentions vestigial progress parser and concat handling; ensure code references are removed.
- `src-tauri/src/audio/progress/mod.rs`: comment mentions legacy CLI progress parser (already removed); fine to keep as historical note or trim.
- `src-tauri/src/audio/processor/prepare.rs`: comment “Removed legacy concat file creation (shell ffmpeg)”—keep or remove after constants cleanup.
- Docs with CLI analogies (`docs/reports/AAC_advice.md`, `fix_comparison.md`): clarify that CLI strings are illustrative, not used in code.

No `std::process::Command("ffmpeg")` usages found. No shell executor remains.


