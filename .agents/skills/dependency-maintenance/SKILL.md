---
name: dependency-maintenance
description: Audiobook Boss dependency and toolchain maintenance workflow. Use when auditing, updating, or risk-classifying Bun/JS packages, Rust/Cargo crates, Rust toolchains, Homebrew build tools, GitHub Actions pins, supply-chain guardrails, or lockfile refreshes in this repo.
---

# Dependency Maintenance

## Purpose

Maintain ABB dependencies without bypassing supply-chain guardrails or turning routine updates into avoidable break/fix churn. Prefer staged updates, explicit risk classification, and full repo proof.

Use root `AGENTS.md` first. Dependency work is code/config work unless it is strictly prose-only, so final validation normally means `scripts/checks.sh standard`.

## Quick Snapshot

Run the bundled read-only helper when starting an audit or before/after updates:

```bash
bash .agents/skills/dependency-maintenance/dependency_snapshot.sh
```

The helper gathers tool versions, dirty state, `bun outdated`, Bun/Rust audit status, `rustup check`, and workflow action pins. It must not edit lockfiles or install anything.

## Staging Model

Keep these lanes separate unless the user explicitly asks for a combined maintenance pass:

1. Bun binary and package-manager behavior.
2. JS package updates governed by Bun and `minimumReleaseAge`.
3. Rustup/rustc/cargo toolchain updates.
4. Rust manifest or `Cargo.lock` refreshes.
5. GitHub Actions pins.
6. Homebrew build tools such as `ffmpeg`, `pkg-config`, and `fdk-aac`.

State which lanes are in scope before mutating files. Keep unrelated dirty work untouched.

## Bun And JS Rules

- Treat Bun as the project package manager, script runner, and test runner.
- Respect `bunfig.toml` `minimumReleaseAge = 604800`. Do not bypass it without explicit user approval.
- After updating Bun, rerun `bun outdated`; interpret `*` entries as age-gated and leave them alone.
- Use `bun ci` to prove the committed lockfile still installs cleanly.
- Use `bun audit` and `bun pm untrusted` as standard supply-chain preflights.
- Do not use `npm audit` here; the repo intentionally has no npm lockfile, so it fails with `ENOLOCK` and adds noise.

## Rust Rules

- Serialize rustup operations. Do not run `rustup update`, `rustup toolchain install`, or overlapping Cargo operations that auto-install the same toolchain in parallel.
- If updating the repo toolchain, update `rust-toolchain.toml` intentionally and prove `rustc --version`, `cargo --version`, and `rustup check`.
- Expect Rust toolchain updates to surface new Clippy lints because ABB runs Clippy with `-D warnings`.
- Prefer the smallest behavior-preserving fix for legitimate new lint findings. Do not add broad `allow` attributes for routine maintenance lints.
- A `Cargo.lock` refresh may update many transitive crates even when `Cargo.toml` changes little. Transitive crates are dependencies pulled in by direct dependencies such as Tauri, Reqwest, Rustls, FFmpeg wrappers, or image tooling.
- `cargo outdated` may be unreliable with ABB's vendored `ffmpeg-sys-next`. Use `cargo update --dry-run`, `cargo tree --invert <crate>@<version>`, crates.io checks, and `cargo audit -D warnings` instead.

## GitHub Actions Rules

- Treat workflow action major bumps as real residual risk even when local checks pass.
- Local validation can catch syntax and repo impact, but Pages/perf/release workflow behavior is fully proven only by GitHub Actions execution.
- Prefer action pin updates as a separate lane from Rust/JS dependency updates unless the user asks for a full maintenance sweep.

## Validation Order

For a full maintenance pass, prefer:

```bash
bun ci
bun audit
bun pm untrusted
cargo audit -D warnings
scripts/checks.sh standard
```

If IPC/Rust contract shapes change, regenerate/check bindings with the existing repo scripts before trusting UI tests:

```bash
scripts/check-generated-bindings.sh --mode local
```

For docs-only edits to this skill or policy surfaces, use:

```bash
bash scripts/check-context-surface.sh
```

Also validate the skill itself after edits:

```bash
python3 /Users/jstar/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/dependency-maintenance
```

## Risk Classification

Use concrete risk language:

- Low risk: lock/install proof only, no package changes, formatter/linter patch updates after age gate.
- Mild risk: Bun binary updates, JS test/build tooling patches, small direct Rust patches.
- Moderate risk: Rust toolchain pin changes, broad `Cargo.lock` refreshes touching Tauri/WebKit/TLS/FFmpeg-adjacent transitive crates.
- CI-only residual risk: GitHub Actions major bumps after local checks pass.
- High risk: anything that bypasses path safety, fallback policy, generated IPC parity, release artifact truth, or supply-chain guardrails.

Do not call something risky just because it is unfamiliar. Tie risk to a concrete failure mode and the proof that would catch it.

## Handoff

Final reports should include:

- What was updated and what was intentionally left alone.
- Whether `minimumReleaseAge` blocked any JS updates.
- Tool versions after update.
- Security/audit preflight results.
- Full check result.
- Any source fixes required by new toolchain behavior.
- Residual risk that needs CI or release-artifact proof.
