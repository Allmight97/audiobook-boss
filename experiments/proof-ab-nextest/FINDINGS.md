# Proof A/B findings (2026-05-27)

## Setup

- Workspace: `experiments/proof-ab-nextest/` (gitignored)
- Synthetic crate: 27 test binaries (25 integration + lib + bin), 4 tests matching `metadata_intent_validation_contract`
- Run: `bash experiments/proof-ab-nextest/scripts/run-ab.sh`
- Cloud VM cannot build real `audiobook-boss` (GTK/Tauri); use `scripts/run-ab-real-macos.sh` on macOS for magnitude validation.

## Structural result (reproduces the trap)

| Scenario | Process launches (`Running` lines) | Zero-test binaries |
|----------|-----------------------------------|--------------------|
| `cargo test -p … metadata_intent_validation_contract` | **27** | **26** |
| `cargo test -p … --lib metadata_intent_validation_contract` | **1** | **0** |
| `cargo nextest run -p … -E 'test(metadata_intent_validation_contract)'` | **0** (nextest UI) | n/a — runs 4 tests, skips 28 |

Cargo without a target selector always fans out to every test binary and applies the name filter inside each process. That matches the 137s vs 0.29s failure mode on macOS when each binary links the full Tauri/media stack.

## Wall-clock on this VM (lightweight post-warm runs)

Heavy deps (tokio full, serde, regex) added to synthetic lib; still sub-second because link cost is << ABB.

| Scenario | elapsed |
|----------|---------|
| cargo (no target) | ~0.29s |
| cargo `--lib` | ~0.25s |
| nextest name filter | ~0.38s |
| nextest `--lib` + filter | ~0.31s |

**Takeaway:** For focused single-binary work, `cargo test --lib` is the fastest path here. Nextest adds runner overhead on tiny crates but avoids 26 spurious process launches and provides structured summary output (`4 tests run, 28 skipped`).

## Nextest vs Cargo for agent feedback

| Dimension | Cargo + correct `--lib`/`--test` | Nextest |
|-----------|----------------------------------|---------|
| Target-selection trap | Must be encoded in proof router | Filter expressions; still need binary awareness for integration tests |
| Machine-readable output | libtest text (parse-heavy) | JUnit, message-format experimental, `nextest list` |
| Focused run speed (ABB-scale) | Excellent if target selector correct | Comparable when only matching binaries run; verify on macOS |
| Adoption cost | Zero (already in toolchain) | `cargo-nextest` install + config (`.config/nextest.toml`) |

## Current repo footgun

`scripts/proof.sh` `rust-contract` route runs:

```bash
cargo test -p audiobook-boss contract_tests
```

No `--lib` → same multi-binary fan-out as the agent workblock. `rust-private` already uses `--lib`.

## Recommendation for proof-system redesign

1. **Router owns target selection** — agents call `proof focus rust lib <filter>`, not raw `cargo test`.
2. **Fix existing routes now** — add `--lib` to `rust-contract` (and audit any other filtered `cargo test` without selectors).
3. **Defer Nextest** until event/artifact contract exists; then add as optional runner for JSON/JUnit + `nextest list` discovery, not as the first fix.
4. **Validate magnitude on macOS** — run `run-ab-real-macos.sh` against real package; synthetic proves topology, ABB proves seconds.

