# Proof infra A/B: Cargo libtest vs nextest

Temporary experiment workspace (not wired into `scripts/proof.sh`).

## Topology

`synthetic-crate/` mirrors ABB’s multi-binary layout:

- 1 library test binary (includes 4 `metadata_intent_validation_contract_*` tests)
- 1 bin test binary
- 25 integration test binaries (each runs unrelated smoke tests)

## Run

```bash
bash experiments/proof-ab-nextest/scripts/run-ab.sh
```

Results land in `experiments/proof-ab-nextest/results/`.

## Full ABB on macOS

This harness does not replace measuring on the real `audiobook-boss` package (Tauri/GTK). Re-run the user’s commands there after changing runners.
