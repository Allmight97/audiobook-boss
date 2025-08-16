Compiling audiobook-boss v0.1.0 (/Users/jstar/Projects/audiobook-boss/src-tauri)

warning: variable does not need to be mutable

   --> src/audio/media_pipeline.rs:605:25

    |

605 |                     let mut total_samples = out.samples() as usize;

    |                         ----^^^^^^^^^^^^^

    |                         |

    |                         help: remove this `mut`

    |

    = note: `#[warn(unused_mut)]` on by default

warning: variable does not need to be mutable

   --> src/audio/media_pipeline.rs:751:25

    |

751 |                     let mut total_samples = out.samples() as usize;

    |                         ----^^^^^^^^^^^^^

    |                         |

    |                         help: remove this `mut`

warning: `audiobook-boss` (lib) generated 2 warnings (run `cargo fix --lib -p audiobook-boss` to apply 2 suggestions)

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 34.10s

     Running `target/debug/audiobook-boss`

[2025-08-16T20:27:14Z INFO  audiobook_boss_lib] Starting Audiobook Boss application

(base) jstar@RRREEE audiobook-boss % npm run tauri dev

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:54Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps

[2025-08-16T20:34:57Z WARN  audiobook_boss_lib::audio::media_pipeline] Frame size mismatch detected. Encoder expects 1024 samples, got 1152 samples.

[2025-08-16T20:34:57Z WARN  audiobook_boss_lib::audio::media_pipeline] Skipping this frame to avoid encoder error. This may cause audio gaps.

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Processed 13162 packets total

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Input packets processed successfully

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] Flushing decoder frames for: /Users/jstar/Downloads/ABB Tests/01 - Introduction/05 - The Unlived Life.mp3

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Decoder frames flushed successfully

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] ✅ Completed processing file: /Users/jstar/Downloads/ABB Tests/01 - Introduction/05 - The Unlived Life.mp3

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Completed processing input file 5/5

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ All input files processed successfully

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] 🏁 Starting encoding finalization...

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Encoding finalization completed successfully

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::media_pipeline] Audio processing completed with metadata integration

[aac @ 0x13e8ac010] Qavg: nan

[2025-08-16T20:34:57Z INFO  audiobook_boss_lib::audio::processor::finalize] Starting finalize stage metadata writing for: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/99da235d-a048-4d4f-a66a-f37893b86ed3/merged.m4b

===

The app never produced an output file but it does appear to process now.