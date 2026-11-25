# Output Artifact

- Folder claims are process-wide and a created folder is kept until its last
  claimant ends, so cancelling the run that created it cannot strand another
  run's empty folders. Cleanup removes only empty ABB-created paths below
  their existing anchor.
- Each created folder also has a record in the cache
  (`folder_records.rs`), because claims die with a killed process.
- Publication, every tag save on a published output, and every metadata Save
  hold that file's lock (`file_lock.rs`), so an earlier export's tag save
  never overwrites a later export of the same file, and a Save never puts a
  file's old audio back over an export published while it ran. The companion
  PDF commit holds the lock on its requested name while it picks a free
  name.
- Publishing under a new name stages complete content and hard-links it, so
  the final name appears only with complete content and never replaces an
  entry.
- Any hard-link failure except a taken name falls back to an empty placeholder
  plus a rename, because network and FUSE disks report a missing hard-link
  feature with many different errors; the rename is refused unless the
  placeholder is still a zero-length regular file.
- Replacement temps carry a key from the final name's hash and never embed the
  name, so the stale-temp sweep removes only that title's temps and any name
  that passes preflight commits. The sweep runs only after a successful
  publish, and `replace_file` returns `CrossesDevices` before moving an
  existing destination aside.
- A rename alternative that would be too long removes only the rename option,
  so a valid ReplaceExisting still plans.
- Publication fixes final artifact truth: a staged-source cleanup failure
  afterward is a success warning, retried by the cleanup guard.
- Audiobookshelf's folder parser is documented in its
  [Book Library Structure](https://audiobookshelf.org/docs/documentation/libraries/book-library/directory-structure/)
  guide: `Author/Series/Book`, sequence and year syntax, narrator braces. One
  naming example does not prove a scanner supports a layout; check that
  scanner's import. The
  [Plex Audiobook Guide](https://github.com/seanap/Plex-Audiobook-Guide) is a
  community recipe that applies only when that Plex setup is in use.
- Audio resolves the output extension before collision review; naming and
  collision policy take that path, and codec choice stays with Audio.

## Outside facts

- macOS and Windows disks usually ignore case, so `file_lock.rs` folds case in
  the lock key there; two files that differ only in case then wait for each
  other.
- APFS folds Unicode normalization forms together, so two titles whose names
  differ only in form name one path; the second must fail, never replace.
- The stale-temp sweep in `commit.rs` checks `DirEntry::file_type`, which does
  not follow symlinks, so a link or folder that imitates the temp name stays.
