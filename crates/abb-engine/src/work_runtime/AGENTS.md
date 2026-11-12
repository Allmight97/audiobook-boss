# WorkRuntime

- A child's `cancellable` is the only authority for offering title cancel; the
  Work Center adds only that the operation has more than one title.
- Missing child timestamps mean unknown; never derive them from logs or batch
  duration.
- One lock guards each operation's snapshot and its live record. Acceptance
  inserts both together, so a file is never reported free between acceptance
  and the first read; finishing removes the record in the critical section
  that writes the terminal snapshot.
- Terminalizing runs under that lock, but `TitleOutput::end`, event emission,
  and `on_finished` run outside it, because `TitleOutput::end` calls a
  listener that takes the lock.
- Stamp an operation's `revision` under the state lock before mutable access.
- Emission happens after the lock, so updates can arrive out of order; a host
  keeps the newest snapshot per operation and the order with the highest
  revision.
- `follow_concurrency_lock` publishes an update with no changed operation
  whenever `concurrency_locked` changes, so the lock can clear after an
  operation's last update.
- Metadata-save children match progress by `input_index` and carry no
  `job_id`, because a shared one would cross-match in `apply_progress_event`.
- Terminal operations are pruned by terminalization order, so a long run that
  just finished survives its own prune.
- A progress event never changes an operation's status; only `terminalize`
  does. Why: the UI, `unfinished_exports`, and `abb-dev` read a terminal status
  as finished, and a title's cleanup ends after its last progress event.
- Hosts, `abb-dev`, session previews, and the UI read `OperationSnapshot.phase`
  and keep no status list of their own.
