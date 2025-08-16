# Audio Pipeline Refactor Rationale & Strategy Summary
(Companion to `planning/audio_pipeline_refactor_plan.md`)

## Why Change Anything? (Blunt Version)
- 11% of decoded audio samples are currently discarded. That's unacceptable for fidelity and professional credibility, even if most users "won't notice" in speech.
- `media_pipeline.rs` at 878 LOC violates project standards (400 LOC cap) and is hostile to future changes.
- Performance is slower than the prior shell path: excessive logging + per-frame object churn + no frame accumulation + no encoder threading.
- Audio quality headroom left on the table: no VBR option, no controlled quality scaling, only plain CBR with occasional twoloop attempt.

## Core Principles Applied
1. Preserve data first; optimize second.
2. Bound module responsibilities.
3. Provide measurable improvements (no hand-wavy "probably faster").
4. Fail soft on optional enhancements (twoloop, VBR) to avoid regressions.

## Problems → Direct Fix Mapping
| Problem | Impact | Fix | Metric |
|---------|--------|-----|--------|
| Frame truncation (1152→1024) | 11% content loss | Accumulator + tail pad | Loss % ≈ 0 |
| Monolithic file | Slows iteration, raises defect risk | Split into 7 focused modules | Lines/file < 350 |
| Chatty logging | I/O + CPU overhead | Throttle & downgrade | Packets/sec ↑ |
| No threading | Underutilized CPU | Enable encoder frame threading | Elapsed time ↓ |
| Only CBR exposed | Users lack quality choice | Add VBR mode (quality ladder) | Perceived quality ↑ at same file size |
| Unverified twoloop | Unknown quality impact | Log explicit enable/fallback | Deterministic diagnostics |

## VBR vs CBR (AAC Speech Target Rationale)
- Target range 48–68 kbps is industry-normal for mono spoken word (Audible typical ~64 kbps).
- CBR is predictable for size estimation; VBR yields better perceptual quality at same average bitrate.
- Strategy: keep existing CBR slider; add VBR quality 1–5 (mapped to approximate internal quality scale). Quality 3 default.

Approx quality mapping (initial heuristic; refine after measurement):
| Quality | Approx Avg kbps (Mono) | Use Case |
|---------|------------------------|----------|
| 1 | ~48 | Long books, storage constrained |
| 2 | ~56 | Balanced small size |
| 3 | ~64 | Default (Audible parity) |
| 4 | ~72 | Higher clarity for sibilants |
| 5 | ~80 | Maximum speech transparency |

## Safety & Fallbacks
- If VBR option set fails → log warning once → revert to CBR chosen bitrate.
- If twoloop set fails → keep standard AAC-LC silently after warning.
- Accumulator failure (should not happen) would degrade to prior truncation only behind an emergency feature flag (`ABB_ACCUMULATOR=0`) for rapid rollback (temporary during rollout only).

## Metrics & Success Definition
Success requires all:
- Loss percent ≤ 0.02%.
- Throughput ≥ 2× baseline MB/s in Release OR clear profiler evidence bottleneck external (I/O bound).
- Users can toggle CBR/VBR and see accurate size estimates (±10%).
- No regression in metadata or cover art embedding.

## Decomposition Snapshot (Target Architecture)
```
audio/
  media_pipeline.rs (façade)
  plan.rs
  encoder.rs
  decoder.rs
  buffer.rs
  process.rs
  progress.rs
  cover_art.rs
```
Responsibilities are linear: plan → encoder setup → per file decode/resample → accumulator → encode → finalize + metadata.

## Risk & Mitigation
| Risk | Mitigation |
|------|------------|
| Unsafe FFI option names drift | Keep small wrapper; unit test returns success/error; gate with env override |
| VBR quality mapping inaccurate | Log actual output average bitrate (analyze packets) for tuning in later pass |
| Hidden latency from padding | Tail padding is silence; negligible for spoken word (<< 1 frame). |
| Complexity from early big split | Delay split until accumulator & threading stable (Phase order chosen accordingly) |

## Non-Goals (Explicitly)
- Gapless playback metadata (encoder delay) – not critical for concatenated audiobook chapters now.
- Crossfade / DSP processing – out of scope for this pass.
- Multi-codec abstraction – AAC only remains fine.

## Post-Refactor Opportunities
- Add optional Loudness Normalization (EBU R128) pre-encode.
- Implement cover art embedding via native stream only (remove Lofty fallback) once stable.
- Add benchmarking harness with criterion for encode speed.

## What to Look For in Code Review (Checklist)
- No truncation code path remains.
- Accumulator logic simple (no nested >3 levels).
- Functions <55 LOC; comments only where non-obvious (unsafe rationale, option names).
- Single point for unsafe AAC option writes.
- Clear structured log summarizing run at end (samples, duration, mode, threading, twoloop status).

## Immediate Next Action
Implement Phase 1 accumulator + logging throttle. Everything else builds on that.

## Bottom Line
We stop throwing away audio, speed things up, and give users a meaningful quality knob—all while making the pipeline maintainable instead of a liability.
