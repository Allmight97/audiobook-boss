## Public API Strip
- Import metadata boundary symbols from `crate::metadata`, not private child modules.
- Intent symbols: `MetadataIntentPatch`, `PatchOp`, `AlbumSortPatchOp`.
- Outcome symbols: `MetadataOutcomeRequest`, `MetadataOutcomePlan`,
  `NamingMetadata`, `CoverArtPassthroughPolicy`, `plan_metadata_outcome`,
  `plan_metadata_write`.
- Crate-local write-plan support: `MetadataWritePlan`, `AlbumSortWriteAction`.

## Private Cluster
- Files: `intent.rs`, `intent_plan.rs`, `contract_tests.rs`; `mod.rs` owns the public re-export strip.
- The broader metadata boundary owns reader/writer interoperability, tag registry behavior, passthrough, cover-art handling, remux helpers, and container routing.

## Allowed Agent Edits Without Escalation
- Change intent internals when `scripts/proof.sh rust-contract` stays green.
- Preserve `set | clear | noop` semantics across save, processing projection, naming projection, write plans, and cover-art handling.
- Preserve external audiobook tag interoperability and fallback-register discipline for metadata read/write changes.
- Drop FFmpeg probe/remux contexts before calling mp4ameta on the same path or replacing the source file.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip symbol.
- Making clear intent partial, lossy, or dependent on sentinel frontend values.
- Changing canonical/mirrored/compatibility tag precedence, fallback behavior, or container routing without explicit evidence and doc updates.
