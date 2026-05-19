# Specta Route Card

- Upstream: `https://github.com/specta-rs/specta.git` on `main`
- Local subtree: `repos/specta`

Refresh:

```bash
git subtree pull --prefix=repos/specta https://github.com/specta-rs/specta.git main --squash
```

## Use For

- Rust type export behavior, TypeScript generation, serde/type mapping, macro
  behavior, and generated binding expectations.
- Source-level confirmation for ABB's IPC type export path.

## Start Here

- `repos/specta/specta/src`
- `repos/specta/specta-typescript/src`
- `repos/specta/specta-macros/src`
- `repos/specta/specta-serde/src`

## Examples And Tests

- `repos/specta/tests/tests`
- `repos/specta/tests/tests/macro`
- `repos/specta/tests/tests/snapshots`
- `repos/specta/examples/basic-ts`
- `repos/specta/examples/collect`

## Avoid

- Do not assume upstream `main` matches ABB's pinned release candidates.
- Do not hand-edit generated ABB bindings to match upstream examples.
- Do not copy snapshot expectations without checking ABB's exporter command.

## ABB Reconciliation

- Check `src-tauri/Cargo.toml` and `Cargo.lock` for pinned `specta` and
  `specta-typescript` versions.
- Verify generated output through `bun run bindings:check` or the relevant
  focused binding command before changing IPC contracts.
- Use `contract-guardrails` when Specta output affects TS/Rust parity.
