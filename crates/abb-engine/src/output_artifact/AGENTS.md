# Output Artifact Boundary

## Public API Strip

- Hosts import the `pub use` types in `mod.rs` and act through session
  intents. The `pub(crate) use` items are for engine owners, which import
  `crate::output_artifact`, not child modules.
- Pure naming, collision, and review data facts live in
  `abb-output-artifact-core`; this directory owns runtime file I/O and final
  commit behavior.
- The core owns the collision action table (`resolve_action`) and each
  output's verdict (`OutputVerdict`: Proceed, Review, Blocked). `plan.rs`
  passes the claimed-by-this-batch check in as a bool. Review signature checks
  stay in `review.rs`; the session aggregates verdicts into one answer.

## Private Cluster

- The cluster owns artifact path derivation, collision detection, review
  signatures, output-root and parent-dir creation after review plus cleanup of
  the empty dirs ABB created, final artifact commit, destination-adjacent
  replacement temps, and final-sidecar Supplemental PDF commit.
- Empty folders ABB created: a title that ends without publishing removes
  those made for it at once (`end_title`). An unfinished output of any run
  keeps the folder it will write into (process-wide claims). The same owner
  retains created folders across runs until the last claimant ends, so
  cancelling the creating run cannot strand another run's empty folders. It
  removes only empty ABB-created paths below their existing anchor.
- `file_lock.rs`: publication and every tag save on a published output hold
  that file's process-wide lock, so a later export replacing the file is never
  overwritten by an earlier export's tag save. The Supplemental PDF commit
  holds the lock on its requested name while it picks a free name and
  publishes, so two exports of one title never pick the same free name.
- Publishing under a new name (Write, RenameNew, Supplemental PDF) never
  replaces an entry. Same disk: the staged audio is flushed, then
  hard-linked. Another disk, and always for the PDF (its source may be a
  download ABB keeps): the bytes are copied into a flushed sibling temp, then
  hard-linked. Why: the final name appears only with complete content. When
  a hard link fails for any reason but a taken name (a collision), the
  flushed temp claims the name with an empty placeholder (refused if taken)
  and is renamed over it; a crash between the two leaves at most an empty
  file. Why any reason: network and FUSE disks report a missing hard-link
  feature with many different errors, and this path never replaces an entry.
- Replacement temps carry a key from the final name's hash, so the stale-temp
  sweep before a commit removes only that title's temps (`commit.rs`). Temp
  names never embed the final name, so any final name that passes preflight
  commits.
- Preflight (`OutputPlanLedger::resolve`) refuses a path component or preview name
  over 255 UTF-16 units (bytes on Linux) before collision probes
  (`abb_output_artifact_core::check_output_name_lengths`). A rename
  alternative that would be too long only removes the rename option, so a
  valid ReplaceExisting still plans. Companion PDF names:
  `processing/AGENTS.md`.

## Edit Rules

- Successful publication fixes final artifact truth. Staged-source cleanup
  happens afterward through the cleanup guard; failures surface a success
  warning and stay owned for retry, with the published output intact.
- Audio resolves the output extension before collision detection and review.
  Naming and collision policy consume that requested path; codec choice stays
  with Audio.
