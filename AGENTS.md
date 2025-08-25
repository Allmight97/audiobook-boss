AGENTS quickstart (for agentic contributors)

Build/run
- Frontend dev: npm run dev (Vite)
- App dev: npm run tauri dev | RUST_LOG=debug npm run tauri dev
- Production build: npm run app:build (Tauri), or (frontend only) npm run build

Test/lint (Rust in src-tauri/)
- All: (cd src-tauri && cargo test)
- Single test: (cd src-tauri && cargo test <name-fragment>)  # e.g., cargo test path_validation
- Lint: (cd src-tauri && cargo clippy -- -D warnings) | Format: cargo fmt --all -- --check

Repo rules (Cursor/Copilot)
- Follow .cursor/rules/repo-guidance.mdc (alwaysApply). Key: single engine (ffmpeg-next), validate all paths, progress via Tauri events, Lofty for metadata.
- Apply .github/copilot-instructions.md engagement rubric (L6 mindset, terse "Topline → Next steps", minimal diffs, smallest safe change).

Style guidelines
- TypeScript: strict; explicit types; avoid any; PascalCase types, camelCase vars/functions, file names camelCase; small, cohesive modules; prefer pure helpers.
- Rust: snake_case modules, CamelCase types; no unwrap/expect (deny clippy::unwrap_used); return Result<T, AppError> and use ?; keep items non-pub unless needed.
- Imports: group std | third-party | local; no wildcard prelude re-exports; stable ordering (rustfmt/prettier defaults).
- Formatting: rustfmt defaults; TS via Vite/tsc; keep functions ≤55 LOC; prefer guard clauses.
- Errors/logging: map external errors into AppError (src-tauri/src/errors.rs); log context, never leak paths in user-facing errors; validate inputs via audio::path_validation.

Testing notes
- Prefer external tests in src-tauri/tests; UI is manual via window.testCommands; name tests clearly to enable name-fragment filtering.
