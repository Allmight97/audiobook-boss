# Working Session

The session is the titles being prepared, the metadata edits made to them, the
lookup that helps fill those edits in, each title's audio choice, where and how
exports are named, and submitting them as an export or a preview. It behaves
the same whether a window, a test, or `abb-dev` drives it, and it outlives any
one host attachment.

## Public API Strip

- Hosts reach the session through `Engine::session_dispatch`,
  `Engine::session_begin`, `Engine::session_snapshot`, `Engine::cover`, and
  `Engine::open_audio_files`, and receive `EngineEvent::Session`.
- The types are the `pub use` list in `mod.rs`. `Session`, `SessionDeps`, and
  the state modules are engine-internal.

## Interface

- A host sends a `SessionIntent` and gets a `SessionReply`: an outcome and a
  `SessionUpdate`. Every change also reaches hosts as `EngineEvent::Session`:
  before an intent waits on a file or the network, when that work finishes, and
  when the engine changes something on its own. A host that attached while an
  intent was running learns its result from the event.
- A `SessionUpdate` carries only the parts that changed. Each part carries the
  revision of its own last change; a host keeps the newest copy of each part,
  so a reply and an event can arrive in either order.
- An intent runs in two steps. `begin` applies its immediate effect before
  returning, so intents begun in order take effect in order. `finish` waits for
  the reply; dropping that wait never cancels the accepted intent. A host whose
  transport can reorder requests calls `begin` in the order the user acted.
- The metadata part carries `binding`, which advances whenever the form binds
  to a different selection. A host shows typing the engine has not yet
  confirmed only on the form it was typed into. `SetField` and `SetFieldAction`
  carry the binding the host showed; the engine answers `EditLost` and writes
  nothing when the form has since bound elsewhere. Why: an import can select a
  title between a keystroke and the update that tells the host. A cover load
  answers `EditLost` when the form changed while it ran, and `Superseded`
  when only a newer cover choice replaced it.
- Covers are not in a snapshot as bytes. Each view that offers a cover
  carries its address as an opaque string: a source on `AudioFile`, a lookup
  result, an Audible library title, the cover on screen, the preview.
  `cover_address.rs` builds every address and parses it back; a source's
  revision is the session-wide count of cover writes when its own cover was
  last written, and no entry is dropped, so ABB's own Saves never make an address
  name two images. A file retagged outside ABB and imported again keeps its
  address.
  `Engine::cover` serves the addresses a session offered through
  `runtime/cover_flow.rs` and refuses anything else. Loading is
  `crate::cover_service`'s. Reasons and statuses are typed; the host words them.

## Where A Rule Goes

- `state.rs`, its `state/` children, and the modules they use hold every rule
  and do no I/O; the one
  exception is `PlanTicket::resolve`, which reads sources on a blocking thread.
  Each method is one atomic transition. Work that needs a file or the network
  leaves as data (a `ReadTicket`, a `SavePlan`, a `PlanTicket`) and returns as a
  completion the state accepts or drops. `check-known-mistakes.ts` rejects
  direct file I/O in session files other than `runtime.rs` and `runtime/`.
- `runtime.rs` and `runtime/` perform that work. `runtime.rs` holds the intent
  pipeline. `runtime/<x>_flow.rs` performs the I/O and ordering of one
  workflow; its rules stay in the state modules. Their tests are children of
  `runtime_tests.rs` (`runtime/<x>_flow_tests.rs`), so they share its `Rig`.
  Runtime never holds the state lock across an
  await, and `Session::transition` re-derives snapshots (`settle`) after every
  transition. It may read WorkRuntime (`sources_in_use`, `sources_held`) under
  the state lock; WorkRuntime and `TitleOutput` callbacks never take the session
  lock, which keeps that order deadlock-free.
- Reads that gate a removal or a save plan (the staged-download sweep,
  `sources_in_use`) run under the state lock, so no submission reserves a file
  between the read and the removal or plan. The sweep and the deferred-writer
  flags live in `SessionState` (`Claim`) and are taken and cleared under the
  same lock; a transition that frees a download afterwards starts the next
  sweep. Waiting writes clear under the state lock, where Save adds them, so a
  new write always finds a writer or starts one. A task holds each such claim
  (and a kept location) as an `InFlight` guard: the task ends it inside the
  transition that decides to stop, and a task that stops early ends it on drop.
  A Save is running exactly while `WriteQueue` holds a file it writes.
- A preview holds concurrency fixed. `Preview` owns its run (accepted, running,
  finished) and the cancel tokens: one parent, one child per title, created in
  the same turn that admits the run, so shutdown either sees it to cancel or the
  preview sees shutdown. A cancel asked for before the run starts is applied to
  the tokens when it starts.
- A download finishing while the list is locked waits for the unlock.
  Source paths are canonical, so match both spellings of the staged root.
  Tag-cache tokens and revisions keep counting across Clear All, so nothing
  issued earlier matches afterward.
- A new rule belongs in the `state/` child that owns its concept, tested in
  that child's `_tests.rs` through the `Desk` harness in `state_tests.rs`. It
  goes in `runtime.rs` or `runtime/` only when it is about ordering between an
  intent and its I/O.
  - `bound_form.rs`: the form bound to the selection and the draft gate.
  - `save.rs`: where each pending edit goes, what a finished write changes,
    and the status a Save ends with (`finish_save`).
  - `write_queue.rs`: which files are being written or wait to be written.
    Its fields are private; `save.rs` and `submitting.rs` ask it.
  - `submitting.rs`: the one submission in flight (`Submission`: idle,
    preparing, awaiting review, previewing). The list lock,
    `submission_in_progress`, the shown review, and the held sources derive
    from it; only `SessionState::enter` writes the `WorkingSet` lock. Also the
    restart answers.
  - `cover.rs`: the cover on screen and each source's cover address.
  - `lookup.rs`: the lookup's status, queue, and supersession.
  - `title_audio.rs`: each title's audio choice and background plan.
  - `snapshots.rs`: the published parts and their revisions.
  Fields shared across children (`status`, `form`, `bound`, `submitting`,
  `audio_refusal`) stay on `SessionState`; grep `state/` before changing one.

## Rules That Are Easy To Break

- **Draft gate.** Selecting, removing, clearing, grouping, and separating first
  accept the edits on screen: validate them and stage them onto every bound
  title that takes edits. An invalid edit refuses the change and leaves the selection where
  it is. Nothing changes the selection while a Save runs.
- **Edit intent.** Only fields the user changed, or blanked, become set or clear
  operations; an untouched field stays absent and is neither rewritten nor
  revalidated. Editing the title sets the album to the same value. Text typed
  after Blank replaces the Blank. One row of `FIELDS` in `metadata_form.rs`
  maps a form field to its stored tag, its edit-patch slot, and its lookup
  result; a new field is a new row, and a const check fails the build when the
  table and `MetadataField::ALL` disagree.
- **Known tags.** The form shows the file's known tags with pending intent
  applied (`tag_cache.rs`), so it never shows a value that Save or processing
  would not send, and never contradicts the file. A source read begun before a
  save, a removal, or Clear All cannot land afterward. Saved values without a
  source read are partial knowledge: an unknown source value cannot justify
  dropping a Blank.
- **Titles.** A title is one or more ordered sources and keeps its identity
  (`input_id`) through reorder, sort, grouping, and separation. Removal names
  that identity, so a repeated click cannot remove the next title. Selection
  names that identity, so a queued reorder cannot retarget a click. Drafts for
  hidden sources survive grouping, and a grouped title's draft is kept for its
  output and never written into a source.
- **Import.** One import runs at a time, in the order the imports were accepted
  (`import_order.rs`), whatever order their tasks start. While the list is
  locked, an import (and analysis that overlaps a submission) waits and
  appends after unlock. A path that discovery skips (`audio/AGENTS.md`, Import Discovery)
  is listed with its reason and the rest is added; the `SkippedPaths` notice
  is set in the transition that lists the titles, after `append_analyzed`
  (which clears the notice).
- **Title totals.** `titles.titleTotals` (by title id), `titles.totalSizeBytes`,
  and `selection.canGroup` are computed in `title_totals.rs` and `WorkingSet`,
  and the list renders them. A title's chapter count is the chapters ABB sends
  to the output: the sources' accepted chapters, or one per source when a
  multi-source title has none (`metadata/chapter_count.rs`, shared with the
  merge and passthrough writers). Per-source rows keep their own count. These
  are packaging facts, not a statement that every container keeps them.
  Every listed title and source carries a `status` (`ready`, `failed`,
  `removedFinished`; `title_status.rs`). A title takes its sources' combined
  status, a failed source outranks a removed one, and its `error` then reads
  as that source's. A CUE source lists the `actions` `choose_cue` accepts for
  its status; the engine refuses any other choice.
  `titles.previewLength` spans the valid titles' preview lengths
  (`preview_config::title_seconds`), so the Preview Audio tooltip names the
  length processing renders.
- **Cover.** A cover change applies only when exactly one valid title is
  selected. A later cover choice or Clear supersedes a load still running.
- **Save targets.** Save covers every pending edit on a valid single-source
  title, not only the selection. A file is busy while an accepted export reads
  it (queued exports included), while a submission or preview being prepared or
  run holds it, or while a waiting write is writing it. A busy local source
  waits and is written when it is free; a temporary download (under the
  remote-source staging root) is never written and its edit stays pending for
  its exports; any other file is written at once and never moved. Two writes to
  one file never overlap.
- **Collision review and restart offers.** `output.collision_review` is the held
  question, independent of a later request's refusal. Its `review_id` advances
  for each review and survives Clear All; choice and cancel intents name it, and
  stale or duplicate answers are `Superseded`. Frontend teardown is not a
  user cancellation. `output.submission_in_progress` owns the busy
  fact. The engine refuses a concurrency change while a job or an export is
  active (`processing/job_registry`) and publishes that lock as
  `concurrency_locked` in the work operations (`work_runtime/AGENTS.md`).
  `output.restart_prompt` selects one unanswered current offer in title-id order
  only after the preceding answer's work settles; answering suppresses that
  offer's automatic question for the engine session, even if Restart or Keep
  fails. Unanswered questions reattach; retained offers stay retryable in Work
  Center. Changing naming makes the old question ineligible. The session owns
  the asked set, the review loop, and the restart queue, so a replaced frontend
  cannot release file work.
- **Exported titles.** A title links to its latest export's output
  (`exports.rs`) while it stays listed. Every Save sends each linked output the
  edit it should carry: what was written to the title's source since the export
  was accepted plus the edit still pending, so an earlier Save is never undone.
  A finished output is retagged in place and never moved. An edit that would
  move an unpublished output is not applied; the output part lists a
  `RestartOffer`. `RestartTitle` (naming its revision) submits that title alone,
  holding its sources and the list from before the old title is cancelled until
  the new export is accepted or the review is abandoned. A title that published
  first is retagged in place instead. `KeepTitleLocation` lets the export
  finish and suppresses the same offer. A later Save, or a new output folder or
  naming, makes an offer stale.
- **Waiting writes.** A waiting write outlives removal of its title.
  `Engine::shutdown` cancels the exports, review, or preview holding it back, so
  it is written before the engine stops. When it finishes, the session reports
  how many were written or failed. If its title is loaded again by then, a
  written edit becomes the title's known tags and a failed one is pending again
  for Save. A failed write stays recorded until its title returns and Save
  retries it; no background retry runs.
- **Listed names.** `AudioFile::label` (title tag, else file name) and
  `artist` are what the list, refusals, the lookup queue, and the draft label
  show. Analysis and
  every successful write (`WorkingSet::note_saved_names`) set them; the list
  does not follow staged edits. The frontend renders them as given. A write
  also sets the source's size (`note_tag_write`).
- **Audio choice.** A title edit applies to every named title or none: one
  title that refuses it, or a locked list, changes nothing and sets
  `audio.refusal` until the selection or the lock changes. A defaults edit is
  recorded in the settings; a title edit is not. A settings reset returns the
  defaults and output choices to the reset settings, except a part the user
  changed after the last change settings handled (`DefaultEdits` against the
  reset snapshot's stamp, `app_settings/AGENTS.md`); loaded titles keep their
  choices. `audio.selection` combines the selected titles: the first one's
  choice, only the options every one accepts, the fields that differ, and what Auto
  resolves to for sample rate and channels: one value, `differs`, or
  unresolved, with the plan problem. A mixed field reads plain Auto and still
  carries the plan problem. Each title carries the same readings for itself.
  `facts` publishes every option list a control shows (formats with their
  codec family, intents, channels, native speeds, encoders) and the bitrate the
  request carries, so a host words and lists them and derives none.
- **Title plans.** Each title's plan resolves in the background whenever its
  request or what a plan reads from its sources changes; a stale result is
  dropped (`plans.rs`). A tag Save changes nothing a plan reads, so a resolved
  plan stays resolved; any other plan is read again, since its
  fingerprint check may have seen the file during the write. That check is the
  only plan step that touches the file; the codec family and copy facts come
  from inspection. A title `FileListInfo::check_cue_chapters` refuses
  (`audio/AGENTS.md`) fails the plan with that reason.
- **Output.** The output part carries the directory, naming, the naming
  processing receives (an empty custom template names `{author}/{title}`), and
  the path the first selected (or first valid) title would get with the values
  on screen. Output choices are recorded in the settings; template typing is
  recorded once it pauses.
- **Preview.** `output.preview_run` owns stable identity from acceptance through
  terminal state, and a finished run stays published until the next submission
  begins, accepted or refused; that ends its claim on the output and cover too.
  `CancelPreview` cancels the run's tokens, which exist only once the preview
  runs; cancellation survives a dropped host
  reply and never grants an output for opening. Accepted custom cover or
  preserved source artwork is read by run identity; bytes stay outside
  snapshots. Only a successful single output can be claimed once through
  `TakePreviewOutput`, across hosts. The variant is `#[serde(skip)]`: it is
  not in the generated type and a webview payload naming it fails to decode.
- **Remote.** `SessionIntent::Remote` routes remote choices and accepted work to
  `remote_source`, which owns the remote, library, and acquisition parts. They
  survive frontend replacement; hosts do not rebuild plans, batch policy, or
  connection drafts. Authorization URLs exist only in RemoteAuthStarted's
  initiating reply.
- **Submission.** `Submit` and `Preview` accept the edits on screen, then build
  the export from the session (`submission.rs`); `SubmitRefusal` lists the
  refusals. An export refuses a valid title with no title tag and no typed title
  (`MissingTitle`); the file name never becomes a title. A preview needs no
  title. From acceptance until the export is registered with WorkRuntime (or the
  preview ends) its sources are held and the list is locked. Outputs that
  already exist hold it at `ReviewRequired` until identified
  `ChooseCollisionPolicy` or `CancelCollisionReview`. The choice applies only to
  the collisions the user saw: one that appears meanwhile sends the submission
  back to review, and execution still rejects a plan whose signature changed
  after approval. After `Engine::shutdown` a submission is refused as `Closing`.
- **Staged downloads.** A finished acquisition's files are imported by the
  session itself (`Session::handoff`), and its titles are recorded as staged in
  the same transition that lists them (`staged.rs`). A download is removed a
  whole acquisition at a time once every title from it completed an export
  without a companion warning (a title's display-only `warnings` do not count)
  or left the list (hidden grouped sources count as
  listed), and no unfinished export, submission, or Save writing holds its
  files. Files being removed count as busy: Save holds them and a submission
  using them is refused. Every transition that leaves a download removable
  starts the one sweep (`Session::transition`), the only path that removes
  downloads. Removing a download leaves the form alone: its title
  (`AudioFile::takes_edits`) keeps its tags and any typing, valid or not, and a
  later Save sends the edit to the title's output. A failed removal stays
  recorded; the sweep sleeps until the retry delay has passed and retries
  with no other change (shutdown ends the wait). The delay starts at
  `staged::RETRY_DELAY`, doubles after each failure up to
  `staged::MAX_RETRY_DELAY`, and logs at warn only when it changes; startup
  clears the rest. A download nothing was
  imported from is recorded the same way. A download with any file that
  cannot be read is not listed at all (`HandoffRefusal::ImportFailed`), so a
  title never lists without one of its parts.
- **Lookup.** A new lookup action supersedes the one in flight; a late search,
  cover, or selection result changes nothing. Open, close, search, skip, apply,
  and editing the query, author, or source each supersede it; changing the apply
  mode or the cover-replace choice does not. `state/lookup.rs` owns each of
  these transitions. Apply
  names the lookup part's revision; a mismatch changes nothing. A result applies
  only to the queued title while it is the one title both selected and bound.
  Applied values are form edits and pass the draft gate like any other.

## Proof

- Rules: `state/<concept>_tests.rs` through the `Desk` harness in
  `state_tests.rs`. `Desk` calls the same `SessionState` steps as the runtime
  and stands in only for the disk and the writer; a status or count it needs
  comes from the step, never from a copy. The property test is
  `state/sequence_tests.rs`; add a new sequence law there as an assertion.
- Real files through `Engine`: `crates/abb-engine/src/test_cases/integration_session_tests.rs`
  and `integration_library_readback_tests.rs` (what a library reads from the export).
- Saves through the session: `integration_session_save_tests.rs`.
