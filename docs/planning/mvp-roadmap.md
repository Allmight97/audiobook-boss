### MVP Roadmap

Goals focus on a working, reliable audiobook merge tool with clear progress, basic metadata, and stable UX. Security hardening and ffmpeg-next migration are deferred but prepared for.

#### Scope (MVP)
- Merge multiple audio files to a single `.m4b`
- Basic metadata write: title, author, album, optional cover art
- Progress UI with cancel; informative errors
- Frontend: file import, order, settings form; status updates

#### Non-Goals (MVP)
- ffmpeg-next migration (prepare boundary only)
- Advanced audio editing or filters
- Extensive security features beyond minimal guardrails

#### Acceptance Criteria
- Process succeeds on supported inputs (`mp3`, `m4a/m4b`, `aac`, `wav`, `flac`)
- Cancel stops processing promptly without orphaned processes
- Output file exists, playable; metadata present when provided
- UI reflects stages and ETA; errors are actionable

#### Milestones
1) Baseline stabilization
   - Tests green (done)
   - Clippy clean for tests and core modules (in progress)
   - Size budget tooling available
2) Test hygiene and structure
   - Extract inline tests from implementation into `src-tauri/tests/**`
   - Replace `unwrap/_err` in tests with `expect/_err`
3) Module trimming (no behavior change)
   - Split `audio/processor.rs` into `prepare.rs`, `execute.rs`, `finalize.rs`
   - Split `audio/progress.rs` into `progress_parser.rs`, `progress_reporter.rs`
   - Split `audio/cleanup.rs` into `cleanup_guard.rs`, `cleanup_ops.rs`
   - Split `commands/mod.rs` into domain-specific modules
   - UI: split large TS modules into state/DOM/actions
4) FFmpeg boundary
   - Introduce `MediaProcessor` trait and `ShellFFmpegProcessor` adapter
   - Add feature flag `safe-ffmpeg` placeholder (no default change)
5) Minimal guardrails
   - Centralize safe path formatting for concat lists
   - Input validation reused across callsites

#### Risks and Mitigations
- Refactor regression: protected by existing integration tests and added unit coverage
- Clippy churn: fix incrementally; keep changes scoped to tests or pure refactors
- Timeboxing: enforce size-budget checks pre-commit

#### Rollout
- Small, focused edits; run tests and clippy each step
- Keep runtime behavior unchanged until boundary is in place


