# Harness Verification

`bun run harness:verify` drives the browser against `harness.html`, emits local artifact packets under `.artifacts/harness/`, and fails when UI changes do not map to scenario coverage or when the page emits runtime errors.

This file covers the required scenario-verification lane only. The interactive browser-review lane (`harness:agent`) is optional, stays outside `scripts/checks.sh standard`, and is documented in `docs/browser-harness.md`.

For Audiobook Boss, treat the `harness:agent` lane as desktop-only by default. Alternate viewport diagnostics are opt-in and should not be part of normal completion claims.

Use:

- `bun run harness:verify --changed`
- `bun run harness:verify --scenario metadata-edit`
- `bun run harness:verify --scenario status-processing`
- `bun run harness:verify --scenario output-preview`
