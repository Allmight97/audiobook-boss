# Folder Interoperability

Read for output organization or scanner parsing changes. Repository paths are
relative to the ABB root.

## ABB Naming Owner

`src-tauri/src/output_artifact/naming.rs` adapts the public preview call to
`crates/abb-output-artifact-core/src/lib.rs`. The core owns ABS-default and
custom-template rendering, optional series/subseries folders, year insertion,
sanitization, fallback names, and template safety. Read its implementation and
focused tests before changing naming; do not copy a sanitizer or template
parser into a caller.

Requested/resolved path safety, collision review, and final writes follow
`src-tauri/src/output_artifact/AGENTS.md`. Folder compatibility does not
replace that runtime authority.

## Scanner Evidence

ABS documents `Author/Series/Book` and `Author/Book` organization. Read the
[Book Library Structure guide](https://audiobookshelf.org/docs/documentation/libraries/book-library/directory-structure/)
for the folder parser involved, including author separators, sequence/year
syntax, narrator braces, and optional subtitle parsing. Check the relevant
server settings when validating a naming change.

For Plex-specific organization, identify the user's library type and metadata
agent before choosing a convention. The
[Plex Audiobook Guide](https://github.com/seanap/Plex-Audiobook-Guide) is a
community integration recipe; consult it when that setup is actually in use,
and verify its claims against the affected installation.

Use generated path previews and core tests for ABB rendering behavior. Use the
affected scanner's import result for a compatibility claim; one naming example
does not establish universal ABS/Plex/Apple support.
