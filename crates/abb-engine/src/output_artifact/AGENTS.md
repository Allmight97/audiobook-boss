# Output Artifact Boundary

## Public API Strip

- This strip is engine-internal; the `pub use` list in `mod.rs` is the surface.
  Hosts import output vocabulary and use session intents. Internal callers
  import `crate::output_artifact`, not child modules.
- Pure naming, collision, and review data facts live in
  `abb-output-artifact-core`; this directory owns runtime file I/O and final
  commit behavior.

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
  overwritten by an earlier export's tag save.

## Edit Rules

- Successful publication fixes final artifact truth. Staged-source cleanup
  happens afterward through the cleanup guard; failures surface a success
  warning and stay owned for retry, with the published output intact.
- Audio resolves the output extension before collision detection and review.
  Naming and collision policy consume that requested path; codec choice stays
  with Audio.
- Final artifact writes and replacement policy live here; processor code asks
  this boundary for artifact truth.
- Replace final artifacts with explicit cross-platform semantics.
