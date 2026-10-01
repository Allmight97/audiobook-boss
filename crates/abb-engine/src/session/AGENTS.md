# Working Session

The session is the titles being prepared, the metadata edits made to them,
and the lookup that helps fill those edits in. It behaves the same whether a
window, a test, or `abb-dev` drives it.

## Interface

- A host sends a `SessionIntent` and gets a `SessionReply`: an outcome and a
  `SessionUpdate`. Changes the engine makes on its own, and changes made
  before an intent's file or network work finishes, arrive as
  `EngineEvent::Session`.
- A `SessionUpdate` carries only the parts that changed (titles, selection,
  metadata, lookup). Each part carries the revision of its own last change; a
  host keeps the newest copy of each part. This is what makes a keystroke cost
  about 1 KB instead of the whole title list, and what lets a reply and an
  event arrive in either order.
- An intent runs in two steps. `begin` applies its immediate effect before
  returning, so intents begun in order take effect in order. `finish` does the
  file or network work and may overlap later intents. A host whose transport
  can reorder requests must call `begin` in the order the user acted.
- The cover image is not in a snapshot. The metadata part carries
  `image_revision`; a host fetches the bytes with `Engine::session_cover_art`
  when that changes.
- Reasons and statuses are typed (`InputNotice`, `MetadataStatus`,
  `LookupStatus`, `CoverNotice`). The host words them.

## Where A Rule Goes

- `state.rs` and the modules it uses (`working_set`, `tag_cache`,
  `metadata_form`, `lookup`) hold every rule and do no I/O. Each method is one
  atomic transition. Work that needs a file or the network leaves as data (a
  `ReadTicket`, a `SavePlan`) and returns as a completion the state accepts or
  drops.
- `runtime.rs` performs that work. It never holds the state lock across an
  await, and it calls `settle` after every transition so snapshots are
  re-derived.
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
  processing would not send. A source read begun before a save, a removal, or
  a reset cannot land afterward. Saved values without a source read are
  partial knowledge: an unknown source value cannot justify dropping a Blank.
- **Titles.** A title is one or more ordered sources and keeps its identity
  (`input_id`) through reorder, sort, grouping, and separation. Grouping
  anchors on the first selected title; conflicting audio requests require an
  explicit choice. Drafts for hidden sources survive grouping, and a grouped
  title's draft is kept for its output and never written into a source.
- **Cover.** A cover change applies only when exactly one valid title is
  selected.
- **Save targets.** Save covers every pending edit on a valid single-source
  title, not only the selection. A file no accepted export is reading is
  written at once and never moved. A local source an export is reading is
  written when every accepted export reading it, queued ones included, has
  finished. A temporary download an export is reading is never written; its
  edit stays pending. A failed write keeps the edit pending. Waiting writes
  outlive removal of their title, and `Engine::waiting_metadata_writes` reports
  them so a host can warn before quitting.
- **Lookup.** A new lookup action supersedes the one in flight; a late search,
  cover, or selection result changes nothing. A result applies only to the
  queued title while it is the one title both selected and bound. Applied
  values are form edits and pass the draft gate like any other.

## Proof

- `state_tests.rs`: rules through a `Desk` that answers reads and saves from
  an in-memory disk, plus a property test over generated sequences of edits,
  selection changes, reads, saves, and exports starting and finishing.
  `working_set_tests.rs` has the same for titles and selection.
- `runtime_tests.rs`: ordering between intents and their I/O, lookup with a
  scripted network, and what a host is told along the way.
- `tests/cases/integration_session_tests.rs`: real files through `Engine`,
  including Save while a real export reads the source, and `abb-dev`.
- A new sequence law is cheaper as another assertion in the property test
  than as a new example test.

## Temporary Until The Processing Move

These exist because encoding, output planning, and processing submission are
still frontend-owned. Remove them with that move; do not build on them.

- `Import` and `ImportOpened` take the title's default audio request from the
  host.
- `SetOrderLocked` lets the host lock the list during a preview.
- `StageSelection` and `Engine::session_metadata_intents` hand pending edits
  to the host's processing payload.
