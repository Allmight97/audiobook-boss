# GreenProof

GreenProof is ABB's repo-level proof surface: **mise tasks + native runners + tiny
evidence capture**. It is not a custom test platform.

## Rules

1. **Prefer native test runners** (Vitest, cargo nextest, cargo test with explicit
   targets, policy scripts).
2. The proof layer may **choose commands**, **preserve logs**, and **summarize step
   status**. It must **not** interpret test semantics, classify failures, invent
   event schemas, or replace native reports.
3. **Smallest command surface** that prevents repeated agent mistakes wins.
4. When adding proof infrastructure, show the command it replaces and the mistake
   it prevents. If it does not prevent a repeated mistake, do not add it.

## Agent workflow

| When | Run |
|------|-----|
| During work (narrowest owner) | `mise run rust:lib <filter>`, `mise run rust:integration <target>`, `mise run runtime:contract`, `mise run test:frontend` |
| Before review | `mise run proof` (or `bun run proof`) |
| Fast static loop | `mise run proof:quick` (alias: `mise run check`) |
| Before release | `mise run proof:release` |
| Exploratory / slow | `mise run diagnose:*` — not merge confidence |

## Task map

| Task | Prevents |
|------|----------|
| `rust:lib <filter>` | Package-wide `cargo test -p audiobook-boss <filter>` fan-out (29 binaries) |
| `rust:integration <target> [filter]` | Running all integration binaries when one target suffices |
| `rust:contract` | Contract tests without `--lib` + missing public-strip check |
| `runtime:contract` | Runtime adapter drift without bindings/strips/Vitest contract files |
| `proof:quick` | Skipping fmt/lint/clippy/policy/bindings on tight loops |
| `proof` | Shipping without scripts + Rust + frontend + production build |
| `test:rust` | Ad-hoc workspace `cargo test` without nextest reporting |
| `rust:media-manual:xhe-aac` | Running ignored fixture without `ABB_XHE_AAC_FIXTURE` preflight |

## Toolchain

First-time setup:

```bash
mise trust mise.toml
mise install
```

Pinned in [`mise.toml`](../mise.toml): `bun`, `rust`, and `aqua:nextest-rs/nextest/cargo-nextest`.

## Evidence (optional, high-value routes)

Routes that capture evidence write to `.proof/latest/`:

- `logs/<step>.log` — raw step output
- `reports/nextest-junit.xml` — nextest JUnit (when `test:rust` runs in `proof`)
- `summary.json` / `summary.md` — step status only (no test semantic parsing)

Clear artifacts: `bun run proof:clean` or `rm -rf .proof`.

Fast loops (`proof:quick`, `rust:lib`) run **bare** mise tasks for speed.

## Layer model

```text
mise.toml          entry + tool pins + task DAG
Vitest / nextest / cargo / check scripts   execution
scripts/greenproof/*.sh   evidence tee + summary (not a runner)
```
