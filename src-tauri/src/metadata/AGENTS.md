## Public API Strip
- Import metadata intent symbols from `crate::metadata`, not private child modules.
- Intent symbols: `MetadataIntentPatch`, `PatchOp`, `AlbumSortPatchOp`, `resolve_effective_processing_metadata`, `resolve_naming_metadata`.
- Crate-local write-plan support: `MetadataWritePlan`, `AlbumSortWriteAction`.

## Private Cluster
- Files: `intent.rs`, `intent_plan.rs`, `contract_tests.rs`; `mod.rs` owns the public re-export strip.
- The broader metadata boundary still owns reader/writer interoperability, tag registry behavior, passthrough, cover-art handling, and remux helpers.

## Allowed Agent Edits Without Escalation
- Change intent internals when `cargo test contract_tests` and `scripts/check-public-api-strips.sh` stay green.
- Preserve `set | clear | noop` semantics across save, processing projection, naming projection, write plans, and cover-art handling.
- Preserve external audiobook tag interoperability and fallback-register discipline for metadata read/write changes.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip symbol.
- Making clear intent partial, lossy, or dependent on sentinel frontend values.
- Changing canonical/mirrored/legacy tag precedence, fallback behavior, or container routing without explicit evidence and doc updates.
