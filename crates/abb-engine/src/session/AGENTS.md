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
  confirmed only on the form it was typed into.
- Covers are not in a snapshot. `Engine::cover` serves the kinds
  `cover_request.rs` lists and refuses anything else. Loading is
  `crate::cover_service`'s. Reasons and statuses are typed; the host words them.

## Where A Rule Goes

- `state.rs` and the modules it uses hold every rule and do no I/O; the one
  exception is `PlanTicket::resolve`, which reads sources on a blocking thread.
  Each method is one atomic transition. Work that needs a file or the network
  leaves as data (a `ReadTicket`, a `SavePlan`, a `PlanTicket`) and returns as a
  completion the state accepts or drops.
- `runtime.rs` performs that work. It never holds the state lock across an
  await, and `Session::transition` re-derives snapshots (`settle`) after every
  transition. It may read WorkRuntime (`sources_in_use`, `sources_held`) under
  the state lock; WorkRuntime and `TitleOutput` callbacks never take the session
  lock, which keeps that order deadlock-free.
- A new rule belongs in the state modules with a test in `state_tests.rs`. It
  goes in `runtime.rs` only when it is about ordering between an intent and its
  I/O.

## Rules That Are Easy To Break

- **Draft gate.** Selecting, removing, clearing, grouping, and separating first
  accept the edits on screen: validate them and stage them onto every valid
  bound title. An invalid edit refuses the change and leaves the selection where
  it is. Nothing changes the selection while a Save runs.
- **Edit intent.** Only fields the user changed, or blanked, become set or clear
  operations; an untouched field stays absent and is neither rewritten nor
  revalidated. Editing the title sets the album to the same value. Text typed
  after Blank replaces the Blank.
- **Known tags.** The form shows the file's known tags with pending intent
  applied (`tag_cache.rs`), so it never shows a value that Save or processing
  would not send, and never contradicts the file. A source read begun before a
  save, a removal, or a reset cannot land afterward. Saved values without a
  source read are partial knowledge: an unknown source value cannot justify
  dropping a Blank.
- **Titles.** A title is one or more ordered sources and keeps its identity
  (`input_id`) through reorder, sort, grouping, and separation. Removal names
  that identity, so a repeated click cannot remove the next title. Selection
  names that identity, so a queued reorder cannot retarget a click. Drafts for
  hidden sources survive grouping, and a grouped title's draft is kept for its
  output and never written into a source.
- **Import.** One import runs at a time, in the order the imports were accepted
  (`import_order.rs`), whatever order their tasks start. A Reset drops an import
  still running, including its failure notice. While the list is locked, an
  import (and analysis that overlaps a submission) waits and appends after
  unlock.
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
  for each review and survives Reset; choice and cancel intents name it, and
  stale or duplicate answers are `Superseded`. Frontend teardown is not a user
  cancellation. `output.submission_in_progress` owns the busy fact.
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
- **Audio choice.** A title edit applies to every named title or none: one
  title that refuses it, or a locked list, changes nothing and sets
  `audio.refusal` until the selection or the lock changes. A defaults edit is
  recorded in the settings; a title edit is not. A settings reset returns the
  defaults and output choices to the reset settings; loaded titles keep their
  choices. `audio.selection` combines the selected titles: the first one's
  choice, only the options every one accepts, and the fields that differ.
- **Title plans.** Each title's plan resolves in the background whenever its
  request or sources change; a stale result is dropped (`plans.rs`). A CUE
  awaiting confirmation, or CUE chapters on a merged title, fails the plan with
  a reason.
- **Output.** The output part carries the directory, naming, the naming
  processing receives (an empty custom template names `{author}/{title}`), and
  the path the first selected (or first valid) title would get with the values
  on screen. Output choices are recorded in the settings; template typing is
  recorded once it pauses.
- **Preview.** `output.previewRun` owns stable identity from acceptance through
  terminal state. `CancelPreview` sets actual per-title cancellation flags even
  during preparation/review/scheduler wait; cancellation survives a dropped host
  reply and never grants an output for opening. Accepted custom cover or
  preserved source artwork is read by run identity; bytes stay outside
  snapshots. Only a successful single output can be claimed once through
  `TakePreviewOutput`, across hosts.
- **Remote.** `SessionIntent::Remote` routes remote choices and accepted work to
  `remote_source`. Its snapshot part survives frontend replacement; hosts do not
  rebuild plans, batch policy, or connection drafts. Progress-only updates omit
  `remote_library`. Authorization URLs exist only in RemoteAuthStarted's
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
  without a companion warning or left the list (hidden grouped sources count as
  listed), and no unfinished export, submission, or Save writing holds its
  files. Files being removed count as busy: Save holds them and a submission
  using them is refused. Every transition that leaves a download removable
  starts the one sweep (`Session::transition`), the only path that removes
  downloads. A failed removal stays recorded and is retried at the first change
  after `staged::RETRY_DELAY`; startup clears the rest. A download nothing was
  imported from is recorded the same way.
- **Lookup.** A new lookup action supersedes the one in flight; a late search,
  cover, or selection result changes nothing. A result applies only to the
  queued title while it is the one title both selected and bound. Applied values
  are form edits and pass the draft gate like any other.

## Proof

- Rules: `state_tests.rs`, including its property test. Add a new sequence law
  there as an assertion.
- Real files through `Engine`: `crates/abb-engine/src/test_cases/integration_session_tests.rs`.

## Boundary Changes

- A `pub use` in `mod.rs`, an intent, or a snapshot field changes: regenerate
  bindings and update the frontend adapters in the same change.
