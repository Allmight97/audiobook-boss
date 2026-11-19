# Metadata core

## Outside facts

- A CUE `INDEX 00` marks the pregap, not the chapter start, so `parse_cue`
  takes `INDEX 01`.
- `MetadataIntentPatch`'s `comment`, `track`, and `disk` are compatibility
  and provenance fields (#281): a normal save keeps them, and only an
  explicit intent edits or clears them.
