# Working Session

- A host keeps the newest copy of each `SessionUpdate` part by its revision,
  because a reply and an `EngineEvent::Session` can arrive in either order.
- The engine emits `EngineEvent::Session` before an intent waits on a file or
  the network, so a host that attaches mid-intent still learns the result.
- A host whose transport can reorder requests calls `begin` in the order the
  user acted: `begin` applies the immediate effect, so begin order is effect
  order.
- `SetField` and `SetFieldAction` carry the form `binding` the host showed,
  because an import can select another title between a keystroke and the
  update that tells the host.
- A source's cover revision counts cover writes session-wide and no entry is
  dropped (`cover_address.rs`), so no address ever names two images.
- Runtime never holds the state lock across an await. It may read WorkRuntime
  (`sources_in_use`, `sources_held`) under the state lock; WorkRuntime and
  `TitleOutput` callbacks never take the session lock. That order is what
  keeps the two deadlock-free.
- Reads that gate a removal or a save plan (the staged-download sweep,
  `sources_in_use`) run under the state lock, so no submission reserves a file
  between the read and the removal or plan. Waiting writes clear under the
  same lock where Save adds them, so a new write always finds a writer or
  starts one.
- A preview's cancel tokens are created in the turn that admits the run, so
  shutdown either sees the run to cancel it or the preview sees shutdown.
- Tag-cache tokens, revisions, and `review_id` keep counting across Clear All,
  so nothing issued earlier matches afterward.
- A new rule goes in the `state/` child that owns its concept, tested in that
  child's `_tests.rs` through `Desk` (`state_tests.rs`). It goes in
  `runtime.rs` or `runtime/` only when it orders an intent against its I/O.
- `Desk` calls the same `SessionState` steps as the runtime and stands in only
  for the disk and the writer; a status or count it needs comes from a step,
  never from a copy. A new sequence law is an assertion in
  `state/sequence_tests.rs`.
- The session, not the frontend, owns the collision review loop, the asked
  restart offers, and the restart queue, so a replaced frontend cannot release
  file work.
- A tag Save changes nothing a title plan reads, so a resolved plan stays
  resolved; any other plan is read again, because its fingerprint check may
  have seen the file mid-write.
