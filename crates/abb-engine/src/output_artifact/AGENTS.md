# Output Artifact

- Folder claims are process-wide and a created folder is kept until its last
  claimant ends, so cancelling the run that created it cannot strand another
  run's empty folders. Cleanup removes only empty ABB-created paths below
  their existing anchor.
- Publication and every tag save on a published output hold that file's lock
  (`file_lock.rs`), so an earlier export's tag save never overwrites a later
  export of the same file. The companion PDF commit holds the lock on its
  requested name while it picks a free name.
- Publishing under a new name stages complete content and hard-links it, so
  the final name appears only with complete content and never replaces an
  entry.
- Any hard-link failure except a taken name falls back to an empty placeholder
  plus a rename, because network and FUSE disks report a missing hard-link
  feature with many different errors.
- Replacement temps carry a key from the final name's hash and never embed the
  name, so the stale-temp sweep removes only that title's temps and any name
  that passes preflight commits.
- A rename alternative that would be too long removes only the rename option,
  so a valid ReplaceExisting still plans.
- Publication fixes final artifact truth: a staged-source cleanup failure
  afterward is a success warning, retried by the cleanup guard.
- Audio resolves the output extension before collision review; naming and
  collision policy take that path, and codec choice stays with Audio.
