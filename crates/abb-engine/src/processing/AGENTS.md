# Processing

- `scripts/dev-log-analysis.ts` parses `OperationKind::log_label`; keep the
  labels stable.
- A failed tag write after acceptance (`title_output.rs`) never changes the
  title's export outcome; an unreadable identity after publication settles
  the title as published and refuses later tag updates.
- A companion PDF failure leaves its title Success with
  `supplemental_warning`, because the audiobook is already published.
- `run_job.rs` runs the output-path check and the companion commit on
  `spawn_blocking`, so a slow disk or a held `lock_output_file` never stalls
  an async worker.
- A title's terminal event goes out only after its folders are removed and
  its `TitleOutput` ended (`run/run_dispatch.rs`). Why: an earlier event shows
  a finished title whose staged files still exist; a later one leaves a failed
  row running.
- `update_max_concurrent` works only while the registry is idle, and an
  `ActiveRun` held from an export's acceptance to its end makes the gap
  between two titles not idle. Admission and reconfiguration share one lock:
  admission records the job and clones the semaphore together, and
  reconfiguration checks for tracked jobs and swaps it together.
- A preview is `OutputKind::Preview`, set once when the plan resolves each
  title's output; execution asks `ProcessingContext::is_preview()` and no
  second flag carries it.
- `RunMode` is one value per run, so an export title without an output record
  cannot be built.
- Keep preflight side-effect-free, so a plan that review refuses leaves no
  folders or files behind.
