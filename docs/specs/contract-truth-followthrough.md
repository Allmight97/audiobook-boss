# Contract Truth Follow-Through

## 1. Purpose / Big Picture

This follow-up batch tightens the highest-value low-severity findings left after
landing `#263`, `#265`, `#266`, and the auto-FDK afterburner fix.

The goal is not another broad cleanup sweep. It is to remove the remaining
contract-truth and invariant gaps that are small enough to fix coherently in one
branch without reopening the finished issue batch:

- validate source-adjacent fallback `sunset=` metadata, not just register rows
- make `EncoderType::Auto` impossible at the encoder-construction boundary
- eliminate generated/public bitrate doc drift for the encoder boundary
- audit project docs/comments so repo-wide documentation and commentary reflect
  the current post-session state instead of older assumptions

Good looks like this:

- fallback marker metadata cannot lie quietly next to code
- unresolved `Auto` cannot silently collapse to Native AAC if an invariant is broken
- generated/public encoder docs match the real validated bitrate contract
- the branch lands as a small patch release with explicit changelog notes

## 2. Scope And Constraints

In scope:

- fallback policy enforcement improvement for source-adjacent `sunset=` metadata
- encoder-construction invariant hardening around resolved-vs-unresolved encoder type
- Rust/generated bitrate doc alignment
- project-wide documentation/commentary audit for touched and nearby canon surfaces
- release follow-through for the landed branch (`1.0.10`)

Non-goals:

- do not reopen `#263`, `#265`, or `#266`
- do not widen into a generic fallback-policy redesign
- do not broaden into metadata-intent doc cleanup tracked in `#267`
- do not reopen `#52` or `#256`
- do not redesign encoder selection types beyond what is needed to make the current invariant explicit

Hard constraints:

- keep the fix set compact and local
- preserve the current runtime behavior except where tightening truth/validation
- any generated binding changes must move with their Rust-side source comments
- update changelog and version only once this branch is actually ready to land

## 3. Solution Posture

Chosen posture: local subsystem hardening patch.

Why this posture:

- all three items are real, but none justify a broader redesign
- they live close to existing ownership boundaries:
  - fallback enforcement in `scripts/check-fallback-policy.*`
  - encoder invariant in `src-tauri/src/audio/processor/encoder/*`
  - boundary docs in `src-tauri/src/audio/settings_encoder.rs` and generated TS
- broader type redesign would add churn without materially improving the immediate outcome

Narrower options rejected:

- fixing only the fallback marker validation would leave known invariant/doc drift behind
- fixing only docs would skip the more meaningful enforcement/invariant wins

What would justify broadening later:

- if removing `Auto` from `create_audio_encoder` exposes multiple callers or repeated resolved-type assumptions, a dedicated resolved-encoder type may be worth introducing

## 4. Context And Orientation

Primary files:

- `/Users/jstar/Projects/audiobook-boss/scripts/check-fallback-policy.sh`
- `/Users/jstar/Projects/audiobook-boss/scripts/check-fallback-policy.test.ts`
- `/Users/jstar/Projects/audiobook-boss/docs/fallbacks.md`
- `/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/processor/encoder/context.rs`
- `/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/processor/encoder/common.rs`
- `/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/settings_encoder.rs`
- `/Users/jstar/Projects/audiobook-boss/src/lib/generated/tauri.ts`
- `/Users/jstar/Projects/audiobook-boss/CHANGELOG.md`
- `/Users/jstar/Projects/audiobook-boss/package.json`
- `/Users/jstar/Projects/audiobook-boss/src-tauri/Cargo.toml`
- `/Users/jstar/Projects/audiobook-boss/src-tauri/tauri.conf.json`
- `/Users/jstar/Projects/audiobook-boss/README.md`
- `/Users/jstar/Projects/audiobook-boss/docs/api-map.md`
- `/Users/jstar/Projects/audiobook-boss/docs/fallbacks.md`

Related reminder issue kept out of this branch:

- `#267` Tighten metadata-intent docs for omitted writable fields

Current implementation shape:

- fallback policy fully validates register sunsets and renewals, but source marker
  `sunset=` is presence-only
- encoder selection resolves `Auto` before `create_audio_encoder`, but the
  constructor still accepts `Auto` and maps it to Native AAC behavior
- bitrate runtime truth is correct, but exported/generated docs omit `104` and `120`

Boundary ownership:

- fallback policy: shell script + shell/Bun regression tests
- encoder invariant: Rust encoder setup path
- generated contract docs: Rust source comments and exported bindings

## 5. Plan Of Work

Phase 1: fallback truth

- parse marker-side `sunset=` from the local metadata chunk
- validate it with `is_calendar_date`
- fail when malformed
- add regression coverage that mutates the source marker `sunset=` instead of only the register row

Phase 2: encoder invariant hardening

- remove the silent `Auto -> Native AAC` fallback at encoder-construction time
- prefer an explicit invariant failure (`unreachable!()` or equivalent error) over permissive behavior
- keep the current `resolve_encoder_type()` routing model
- only broaden to a resolved-encoder type if local changes show that the simpler fix leaves ambiguity

Phase 3: boundary doc truth

- fix the bitrate doc comment in Rust to include `104` and `120`
- regenerate bindings so `src/lib/generated/tauri.ts` matches the Rust source

Phase 4: repo-wide documentation/commentary audit

- audit canon docs and nearby comments for stale statements exposed by today’s work
- specifically check:
  - fallback-policy commentary and examples
  - encoder-selection / FDK / afterburner wording
  - generated-boundary commentary that may still imply stale bitrate truth
  - any nearby docs/comments that still describe pre-`#263/#265/#266` state where
    this branch would now make that wording misleading
- update only high-ROI surfaces; do not sprawl into a generic docs rewrite

Phase 5: release closeout

- bump ABB version to `1.0.10`
- add one compact changelog entry for:
  - fallback marker sunset validation
  - encoder `Auto` invariant hardening
  - bitrate doc/binding alignment
  - any meaningful canon doc alignment done as part of this branch

Sub-agent lane breakdown:

- lane A, optional mini explorer: fallback script/tests only
- lane B, optional mini explorer: encoder invariant path only
- orchestrator owns:
  - final code changes
  - binding regeneration
  - changelog/version follow-through
  - final validation and landing

Reasoning:

- the write sets are small, but the actual code edits are tightly coupled enough
  that the orchestrator can likely do them faster than parallel writers
- targeted explorers are useful for confirmation, not for code ownership

## 6. Progress

- 2026-04-20: post-landing audits identified three follow-up items worth considering:
  source marker `sunset=` validation, dead `Auto` encoder arm, bitrate doc drift
- 2026-04-20: reminder issue `#267` created for separate metadata-intent doc cleanup
- 2026-04-20: this spec created to cluster items 1, 2, and 3 into one coherent follow-up branch
- 2026-04-20: implemented marker-side `sunset=` calendar validation plus regression coverage
- 2026-04-20: hardened resolved-encoder invariant so encoder construction rejects unresolved `Auto`
- 2026-04-20: updated Rust/generated bitrate docs to include `104` and `120`
- 2026-04-20: completed repo-wide blast-radius doc audit; no canon drift remained beyond the touched fallback docs
- 2026-04-20: bumped release surfaces to `1.0.10` and passed `scripts/checks.sh standard`

## 7. Surprises And Discoveries

- the strongest remaining behavior gap is not in runtime fallback expiry logic; it is
  that the source-adjacent marker metadata can still be malformed while the register stays valid
- the `Auto` encoder issue is not live in current flow, but it is still worth fixing because
  its fallback behavior is semantically wrong if ever reached
- generated boundary docs remain a real drift surface even when runtime types are otherwise aligned

## 8. Decision Log

- keep metadata-intent doc cleanup out of this branch and track it in `#267`
  because it is reminder-grade doc truth, not part of the more compelling enforcement/invariant cluster
- treat the three active items as one branch because they are all contract-truth follow-through
  and should reasonably ship together as one patch release
- prefer orchestrator-owned implementation over parallel writer agents for this branch
  because the total write set is small and cross-checking is cheap

## 9. Validation And Acceptance

Required validation:

- targeted fallback tests:
  - `bash scripts/check-fallback-policy.sh`
  - `bun test scripts/check-fallback-policy.test.ts`
- targeted Rust validation for the encoder invariant path:
  - focused `cargo test` if new/updated test coverage is added
- binding alignment check if generated files change:
  - `scripts/check-generated-bindings.sh --mode local` or the repo-standard path inside `scripts/checks.sh standard`
- docs/commentary audit acceptance:
  - touched canon docs and nearby repo comments must reflect current behavior
  - stale pre-fix wording discovered during this branch must be corrected before landing
- full gate:
  - `PATH=/opt/homebrew/bin:$PATH scripts/checks.sh standard`

Acceptance:

- marker-side malformed `sunset=` fails with a clear message and test coverage exists
- encoder construction no longer accepts unresolved `Auto` as a quiet Native AAC fallback
- generated/public bitrate docs match runtime truth
- repo-wide documentation/commentary in touched blast radius reflects current state
- changelog and version are updated for `1.0.10`
- branch is review-ready and synced

## 10. Interfaces And Dependencies

User-visible behavior:

- no intended UI workflow change
- fallback-policy failures become stricter for malformed source marker sunsets

Contract/doc surfaces:

- encoder bitrate docs in Rust and generated TS
- fallback marker metadata expectations in code comments and tests

Tooling/docs that must stay aligned:

- `scripts/check-fallback-policy.sh`
- `scripts/check-fallback-policy.test.ts`
- generated bindings
- `README.md`, `docs/api-map.md`, and `docs/fallbacks.md` when affected
- `CHANGELOG.md`
- package/app version files

## 11. Idempotence And Recovery

Safe restart points:

- fallback script/tests can be rerun repeatedly without side effects
- encoder invariant fix is local to Rust and can be revalidated with targeted tests and the full standard gate
- binding regeneration can be rerun safely

If interrupted:

- resume from this spec
- inspect `git diff --stat`
- rerun targeted checks for the touched phase before returning to the full standard gate

## 12. Completion And Cleanup

This spec can be deleted when all of the following are true:

- items 1, 2, and 3 are implemented and validated
- project-wide documentation/commentary audit for this branch’s blast radius is complete
- `1.0.10` version/changelog work is landed
- branch is merged/synced
- any audit/review findings for this branch are resolved

After completion:

- delete this spec file
- keep `#267` open until its doc-only reminder work is actually handled
