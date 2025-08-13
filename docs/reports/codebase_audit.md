### Code Index Analysis and Context7 Cross‑Check (2025-08-13)

This report combines a repository-wide module/function scan with a Context7 cross-check of referenced libraries. No code was modified.

### Module implementation line analysis

Raw output from the existing analyzer shows per-file totals and implementation lines (comments/tests/blanks excluded from Impl). Files with implementation lines > 400 are flagged.

```text
Module Implementation Line Analysis
================================================================================
Module                                   Total  Comments Blank  Test   Impl   Ov
er 400?
--------------------------------------------------------------------------------
src-tauri/src/audio/media_pipeline.rs    727    84       78     0      565    YE
S
...auri/src/audio/progress_monitor.rs    364    39       38     0      287    NO
src-tauri/src/audio/cleanup/guard.rs     349    25       41     0      283    NO
src/ui/statusPanel/logic.ts              372    37       57     0      278    NO
...uri/src/audio/progress/reporter.rs    400    46       46     54     254    NO
src-tauri/src/audio/context.rs           337    56       41     0      240    NO
src/ui/outputPanel.ts                    361    78       54     0      229    NO
src-tauri/src/audio/file_list.rs         400    34       50     87     229    NO
src/ui/fileList/actions.ts               261    17       47     0      197    NO
...uri/src/audio/processor/prepare.rs    215    35       30     0      150    NO
src-tauri/src/ffmpeg/command.rs          240    17       41     40     142    NO
src/ui/fileList/dom.ts                   178    10       30     0      138    NO
src-tauri/src/audio/settings.rs          298    24       40     106    128    NO
src/ui/coverArt.ts                       211    60       30     0      121    NO
src-tauri/src/audio/mod.rs               174    48       17     0      109    NO
src-tauri/src/commands/audio.rs          150    19       22     0      109    NO
...ri/src/audio/processor/finalize.rs    161    56       9      0      96     NO
src/ui/statusPanel/dom.ts                175    57       25     0      93     NO
src/ui/fileImport.ts                     108    1        18     0      89     NO
src-tauri/src/commands/metadata.rs       121    14       18     0      89     NO
src/main.ts                              112    14       10     0      88     NO
src/types/audio.ts                       106    3        17     0      86     NO
src-tauri/src/metadata/writer.rs         134    6        22     26     80     NO
src-tauri/src/audio/processor/mod.rs     149    62       15     0      72     NO
src-tauri/src/audio/constants.rs         173    60       42     0      71     NO
...auri/src/audio/processor/legacy.rs    158    82       8      0      68     NO
src-tauri/src/ffmpeg/mod.rs              108    23       18     0      67     NO
...uri/src/audio/processor/execute.rs    106    26       14     0      66     NO
...tauri/src/audio/path_validation.rs    211    15       33     98     65     NO
src-tauri/src/audio/metrics.rs           84     15       10     0      59     NO
src-tauri/src/audio/cleanup/ops.rs       70     2        10     0      58     NO
...tauri/src/audio/progress/parser.rs    82     4        10     18     50     NO
src/ui/fileList/events.ts                63     8        9      0      46     NO
src-tauri/src/audio/session.rs           72     17       11     0      44     NO
src-tauri/src/metadata/reader.rs         77     5        15     18     39     NO
src-tauri/src/metadata/mod.rs            57     15       6      0      36     NO
src-tauri/src/lib.rs                     57     3        9      10     35     NO
src-tauri/src/errors.rs                  51     4        13     0      34     NO
src/types/events.ts                      197    142      22     0      33     NO
src/ui/fileList/state.ts                 39     4        9      0      26     NO
src/types/metadata.ts                    54     25       4      0      25     NO
src/ui/fileList/index.ts                 32     4        4      0      24     NO
src-tauri/src/commands/system.rs         25     6        5      0      14     NO
src-tauri/src/audio/progress/mod.rs      21     5        4      0      12     NO
src-tauri/src/commands/mod.rs            9      0        3      0      6      NO
src-tauri/src/audio/cleanup/mod.rs       9      0        4      0      5      NO
src-tauri/src/main.rs                    6      1        1      0      4      NO
src/ui/statusPanel.ts                    8      6        1      0      1      NO
src/ui/fileList.ts                       3      0        2      0      1      NO
src/ui/statusPanel/index.ts              8      6        1      0      1      NO
src-tauri/src/tests_integration.rs       411    69       72     270    0      NO

================================================================================
SUMMARY: 1 modules exceed 400 implementation lines

Modules exceeding 400 implementation lines:
  • src-tauri/src/audio/media_pipeline.rs: 565 lines
```

- Only `src-tauri/src/audio/media_pipeline.rs` exceeds 400 implementation lines.
- Commentary share in that file is ~11.6% (84/727); blanks ~10.7% (78/727). Not commentary-bloated by a >30% threshold.

### Functions over limits

Automated scan for Rust/TS flagged functions with body lines > 50 or parameters > 7. Results below (JSON):

```json
[
  {
    "file": "src-tauri/src/tests_integration.rs",
    "flagged_functions": [
      { "name": "test_current_audio_processing_flow", "params": 0, "lines": 54, "start_line": 64 },
      { "name": "test_file_validation", "params": 0, "lines": 68, "start_line": 253 }
    ]
  },
  {
    "file": "src-tauri/src/audio/file_list.rs",
    "flagged_functions": [
      { "name": "test_debug_real_mp3_file", "params": 0, "lines": 64, "start_line": 251 },
      { "name": "test_debug_lofty_m4b_errors", "params": 0, "lines": 83, "start_line": 317 }
    ]
  }
]
```

- Note: All flags are in test functions; no production functions exceeded thresholds.

### Context7 cross‑check and improvement notes

Checked key libraries against Context7 for current practices and guidance.

- Tauri 2 (backend/frontend, commands/events/security)
  - Prefer least-privilege capability permissions. Current `src-tauri/capabilities/default.json` uses `core:default`, `dialog:default`, `opener:default`.
  - Consider explicitly granting only needed permissions for events (emit/listen/unlisten), window controls, path, and tray per Tauri’s permission specs to reduce attack surface.
  - Ensure frontend uses `@tauri-apps/api` v2 patterns (`core.invoke`, `event.emit/listen`) consistently; avoid legacy globals where possible.

- FFmpeg bindings (with practical recommendations)
  - Using `ffmpeg-next` v7 (optional) aligns with safer bindings. Alternative high-level wrappers (e.g., ez-ffmpeg) exist but may not match current control needs; keep present approach unless ergonomics become a blocker.
  - For heavy encode workloads, keep work off the main thread; prefer async tasks and avoid blocking the Tauri runtime.
  - Practical recommendation: Keep dual-engine strategy: default = shell; feature-gated v7 for advanced work and future flip.
  - Ship to public on shell until you have a clean, reproducible packaging story for FFmpeg libs (and licensing) with Tauri.
  - Use v7 where its strengths matter (accurate progress, fine-grained control, future features), and keep parity tests to de-risk a later default flip.
  - Test gates:
    - Shell: cargo test; package via current flow.
    - v7: cargo test --features safe-ffmpeg; verify progress/cancel; confirm packaging plan for FFmpeg libs before making default.

- Tokio
  - Use `spawn_blocking` for CPU-bound sections if any synchronous work leaks into async contexts.
  - Consider `tracing` integration for async observability if deeper telemetry is needed.

- Serde / thiserror
  - Continue deriving rich error types with `thiserror` and use `#[from]` where appropriate for source chaining.
  - Prefer typed `Result<T, AppError>` at boundaries; keep `anyhow` out of production paths.

- Vite
  - Repo uses Vite 6.x; Context7 shows v7 availability. Consider evaluating upgrade when compatible with Tauri 2 toolchain, especially if bundling or HMR improvements matter.

- Lofty (metadata)
  - Verify you’re on a recent stable; keep an eye on tag writing edge-case fixes related to M4B/MP4 atoms.

### Code index status (for reproducibility)

- Index active with file watcher
  - Files indexed: 124
  - Languages detected: javascript, typescript, shell, markdown, python, css, yaml, rust, json, html
  - Settings dir: temporary code-indexer store present

### Appendix: methodology

- Module totals from the existing `line_counter.py` utility.
- Function thresholds via a one-off static scan (Rust `fn`, TS `function` and `const name = (...) => { ... }`).
- Library checks via Context7 for Tauri permissions and ecosystem notes.


