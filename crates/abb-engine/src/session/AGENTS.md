# Working Session

The session is the titles being prepared, the metadata edits made to them,
the lookup that helps fill those edits in, each title's audio choice, where
and how exports are named, and submitting them as an export or a preview. It
behaves the same whether a
window, a test, or `abb-dev` drives it, and it outlives any one host
attachment.

## Public API Strip

- Hosts reach the session through `Engine::session_dispatch`,
  `Engine::session_begin`, `Engine::session_snapshot`, and
  `Engine::session_cover_art`, and receive `EngineEvent::Session`.
- Types are the `pub use` list in `mod.rs`: the intent, outcome, reply, and
  update types, each snapshot part, and the typed statuses and notices.
- `Session`, `SessionDeps`, and the state modules are engine-internal.

## Interface

- A host sends a `SessionIntent` and gets a `SessionReply`: an outcome and a
  `SessionUpdate`. Every change also reaches hosts as `EngineEvent::Session`:
  before an intent waits on a file or the network, when that work finishes,
  and when the engine changes something on its own. A host that attached
  while an intent was running learns its result from the event.
- A `SessionUpdate` carries only the parts that changed (titles, selection,
  metadata, lookup, audio, output). Each part carries the revision of its own last change; a
  host keeps the newest copy of each part. This is what makes a keystroke cost
  about 1 KB instead of the whole title list, and what lets a reply and an
  event arrive in either order.
- An intent runs in two steps. `begin` applies its immediate effect before
  returning, so intents begun in order take effect in order. The engine
  starts and tracks the file or network work before returning; `finish` waits
  for its reply, and dropping that wait never cancels the accepted intent. A host whose transport
  can reorder requests must call `begin` in the order the user acted.
- The metadata part carries `binding`, which advances whenever the form binds
  to a different selection. A host showing typing the engine has not yet
  confirmed shows it only on the form it was typed into.
- The cover image is not in a snapshot. The metadata part carries
  `image_revision`; a host fetches the bytes with `Engine::session_cover_art`
  when that changes.
- Reasons and statuses are typed (`InputNotice`, `MetadataStatus`,
  `LookupStatus`, `CoverNotice`, `SubmissionStatus`). The host words them.

## Where A Rule Goes

- `state.rs` and the modules it uses (`working_set`, `tag_cache`,
  `metadata_form`, `lookup`, `audio_choice`, `audio`, `plans`, `output`,
  `submission`, `staged`, `exports`) hold every rule and do no I/O; the one exception is
  `PlanTicket::resolve`, which reads sources and runs on a blocking thread.
  Each method is one atomic transition. Work that needs a file or the network
  leaves as data (a `ReadTicket`, a `SavePlan`, a `PlanTicket`) and returns
  as a completion the state accepts or drops.
- `runtime.rs` performs that work. It never holds the state lock across an
  await, and it calls `settle` after every transition so snapshots are
  re-derived. It may read WorkRuntime (`sources_in_use`, `sources_held`)
  under the state lock; WorkRuntime and `TitleOutput` callbacks never take
  the session lock, which keeps that order deadlock-free.
- A new rule belongs in the state modules with a test in `state_tests.rs`. Put
  it in `runtime.rs` only when it is about ordering between an intent and its
  I/O.

## Rules That Are Easy To Break

- **Draft gate.** Selecting, removing, clearing, grouping, and separating
  first accept the edits on screen: validate them and stage them onto every
  valid bound title. An invalid edit refuses the change and leaves the
  selection where it is. Nothing changes the selection while a Save runs.
- **Edit intent.** Only fields the user changed, or blanked, become set or
  clear operations; an untouched field stays absent and is neither rewritten
  nor revalidated. Editing the title sets the album to the same value. Text
  typed after Blank replaces the Blank.
- **Known tags.** What the form shows is the file's known tags with pending
  intent applied (`tag_cache.rs`), so it never shows a value that Save or
  processing would not send, and never contradicts the file. A source read
  begun before a save, a removal, or a reset cannot land afterward. Saved
  values without a source read are partial knowledge: an unknown source value
  cannot justify dropping a Blank.
- **Titles.** A title is one or more ordered sources and keeps its identity
  (`input_id`) through reorder, sort, grouping, and separation. Removal names
  that identity so a repeated click cannot remove the next title. Grouping
  anchors on the first selected title; conflicting audio requests require an
  explicit choice. Drafts for hidden sources survive grouping, and a grouped
  title's draft is kept for its output and never written into a source.
- **Import.** One import runs at a time. A Reset drops an import still
  running, including its failure notice. Files the OS opened stay queued while
  the list is locked; an accepted OS-open request waits and drains them after
  unlock independently of the host. Analysis that overlaps a submission waits
  before appending to its locked list.
- **Cover.** A cover change applies only when exactly one valid title is
  selected. A later cover choice or Clear supersedes a load still running.
- **Save targets.** Save covers every pending edit on a valid single-source
  title, not only the selection. A file is busy while an accepted export reads
  it (queued exports included), while a submission or preview being prepared
  or run holds it, or while a waiting write is writing it. A busy local source
  waits and is written when it is free; a temporary download (under the
  remote-source staging root) is never written and its edit stays pending for
  its exports; any other file is written at once and never moved. Two writes
  to one file never overlap.
- **Exported titles.** A title links to its latest export's output
  (`exports.rs`) while it stays listed. Every Save sends each linked output
  the edit it should carry: what was written to the title's source since the
  export was accepted plus the edit still pending, so an earlier Save is never
  undone. The output takes it before or after publication
  (`processing/title_output.rs`); a finished output is retagged in place and
  never moved. An edit that would move an unpublished output is not applied:
  the output part lists a `RestartOffer`. `RestartTitle` (naming its revision)
  submits that title alone, holding its sources and the list from before the
  old title is cancelled until the new export is accepted or the review is
  abandoned; a title that published first is retagged in place instead
  (`FinishedBeforeRestart`). `KeepTitleLocation` lets the export finish and
  the same location is not offered again. An offer is stale once a later Save
  replaces it or the output folder or naming changes.
- **Waiting writes.** A waiting write outlives removal of its title.
  `Engine::shutdown` cancels the exports, review, or preview holding it back,
  so it is written before the engine stops. When it finishes, the session reports how many were written or
  failed. If its title is loaded again by then, a written edit becomes the
  title's known tags and a failed one is pending again for Save. A failed
  write remains recorded until its title returns and Save retries it; it does
  not retry repeatedly in the background.
- **Audio choice.** The defaults new titles start from and each title's own
  choice are edited with typed `AudioEdit`s checked against the encoder
  capabilities (`audio_choice.rs`); a refused edit changes nothing. MP3 copies
  its source; editing how audio is encoded selects Encode. A defaults edit is
  recorded in the settings; a title edit is not, and is refused while the
  list is locked. A settings reset returns the defaults and output choices to
  the reset settings; loaded titles keep their choices.
- **Title plans and estimates.** Each title's plan is resolved in the
  background whenever its request or sources change; a stale result is
  dropped (`plans.rs`). A CUE awaiting confirmation, or CUE chapters on a
  merged title, fails the plan with a reason. The size estimate is the
  sources' bytes when kept, or duration times target bitrate plus 3 percent
  when encoded; Auto waits for its plan, and a missing fact means none.
- **Output.** The output part carries the directory, naming, the naming
  processing receives (an empty custom template names `{author}/{title}`),
  and the path the first selected (or first valid) title would get with the
  values on screen. Output choices are recorded in the settings; template
  typing is recorded once it pauses.
- **Submission.** `Submit` and `Preview` accept the edits on screen, then
  build the export from the session: valid titles in list order with their
  ordered sources, audio requests, chapter plans, naming, and pending edits
  (`submission.rs`). An invalid standalone title is left out; a grouped title
  with an invalid source, an unresolved audio choice, a CUE awaiting review,
  no output folder, a Save writing, or another submission in progress refuses
  it with a `SubmitRefusal`. From acceptance until the export is registered
  with WorkRuntime (or the preview ends) its sources are held and the list is
  locked. Outputs that already exist hold it at `ReviewRequired` until
  `ChooseCollisionPolicy` or `CancelCollisionReview`. The choice applies only
  to the collisions the user saw: one that appears meanwhile sends the
  submission back to review, and execution still rejects a plan whose
  signature changed after approval. After `Engine::shutdown` a submission is
  refused as `Closing`.
- **Staged downloads.** A finished acquisition's files are imported by the
  session itself (`Session::handoff`), and its titles are recorded as staged
  in the same transition that lists them (`staged.rs`); the job carries the
  outcome. Exports take each title's companion PDFs from that record. A
  download is removed a whole acquisition at a time once every title from it
  completed an export without a companion warning or left the list (hidden
  grouped sources count as listed), and no unfinished export, submission,
  or Save writing holds its files. Files being removed count as busy: Save
  holds them and a submission using them is refused. Every transition that
  leaves a download removable starts the one sweep (`Session::transition`);
  no other code path removes downloads. Successfully removed sources become
  invalid history rows, so later batches exclude them instead of reopening a
  deleted download. A failed removal stays recorded and is
  retried at the first change after `staged::RETRY_DELAY`; startup clears the
  rest. A download nothing was imported from is recorded the same way, so its
  removal is retried too.
- **Lookup.** A new lookup action supersedes the one in flight; a late search,
  cover, or selection result changes nothing. A result applies only to the
  queued title while it is the one title both selected and bound. Applied
  values are form edits and pass the draft gate like any other.

## Proof

- `state_tests.rs`: rules through a `Desk` that answers reads and saves from
  an in-memory disk, plus a property test over generated sequences of edits,
  selection changes, reads, saves, and exports starting and finishing,
  including failed writes. `working_set_tests.rs` has the same for titles and
  selection.
- `audio_choice_tests.rs` and `plans_tests.rs`: audio edit rules, plan
  resolution and staleness, and size estimates.
- `runtime_tests.rs`: ordering between intents and their I/O, lookup with a
  scripted network, and what a host is told along the way.
- `submission_tests.rs`: what a submission sends and when it refuses.
- `staged_tests.rs`: when a staged download may be removed.
- `exports.rs` tests: the edit an exported output carries and when an offer
  is taken.
- `src/test_cases/integration_session_tests.rs`: real files through `Engine`,
  including submit, collision review, preview, Save while a real export reads
  the source (source and output read back), a finished output retagged in
  place, restart at a new location, shutdown, and `abb-dev`.
- A new sequence law is cheaper as another assertion in the property test
  than as a new example test.

## Boundary Changes

- Adding, removing, or renaming a `pub use` in `mod.rs`, an intent, or a
  snapshot field. Regenerate bindings and update the frontend adapters in the
  same change.
