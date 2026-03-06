# Harness Verification

`bun run harness:verify` drives the browser against `harness.html`, emits local artifact packets under `.artifacts/harness/`, and fails when UI changes do not map to scenario coverage or when the page emits runtime errors.

Use:

- `bun run harness:verify --changed`
- `bun run harness:verify --scenario metadata-edit`
- `bun run harness:verify --scenario status-processing`
- `bun run harness:verify --scenario output-preview`
