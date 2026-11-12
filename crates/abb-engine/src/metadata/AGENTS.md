# Metadata

- For ABS, Plex, and Apple tag mapping, series-tag strategy, and folder
  conventions, use the `audiobook-metadata` skill.
- Real file saves go through `save_metadata_intent`, so series validation can
  see the source's tags.
- Ask `container::Container` which container a file is; do not compare FFmpeg
  format name strings. No check enforces this.
- MP4-family tag truth never depends on a bare remux, because FFmpeg's mov
  muxer drops unknown keys (rustdoc on `finish_artifact_tags`).
- A Save never opens the original for writing: it checks a sibling copy and
  swaps it in, so a crash or a bad write leaves the original whole.
- `tag_support::refuse_unholdable_edits` fails a Save that edits a tag the
  container cannot store, because the muxer drops such a tag and the Save
  would report success.
- The remux read loop ends only at end of file; any other read error fails the
  write, because skipping a packet would save a shorter book.
- An MP4 remux keeps the source's `encoder` tag (the mov muxer writes it from
  `encoding_tool`) and every access unit an edit list hides, because Audio
  recognizes ABB's FAAC HE files by that tag and needs the hidden last unit
  for the delayed tail.
- Matroska pictures are written back as attachments, because its muxer drops
  attached-picture streams.
- Every muxer refuses an unsized picture and the bundled FFmpeg decodes JPEG
  only, so `cover_art::format::picture_dimensions` reads picture sizes from
  headers. An ID3 picture carries no size, so an MP3 Save with a JPEG XL cover
  fails and keeps the original; a JPEG XL reader needs a bit reader and a box
  walk, which ABB does without.
- Inherited odd series tags from external files do not block unrelated intent,
  but touched series intent must produce a round-trippable shape before save
  or processing.
- Sinks write in op order, because FFmpeg dictionary write order is part of
  the file: replacing a key moves the last entry into its slot.
- A source cover that cannot be prepared is dropped with a warning, but a
  cover the user chose fails the title. Why: a damaged source picture must not
  cost the user the book, and a picked cover must not vanish silently.
- A cover is converted only in write-prep (`prepare_output_cover_art`) and in
  `cover_source.rs`, so displays show the source's real bytes.
- MP3 (ID3 APIC) and FLAC take the picture type from the stream's `comment`
  tag, so `add_cover_stream` sets `Cover (front)`; without it the muxers write
  type 0 "Other".
- A save preserves the album sort unless the user changes it, because it
  edits the user's own source file. Processing writes only a derived album
  sort, because an underivable one already equals the source's value.
- FFmpeg names the MP3 album-sort frame `album-sort` and the MP4 atom
  `sort_album`; Matroska and ID3 user text frames report keys in upper case.
  `reader.rs` still reads `sort_album` on MP3 for files an older ABB tagged,
  though no library reads that stray TXXX frame.
- A derived album sort keeps book-number fractions, so novellas sort between
  books.
- The album sort stays its own `Option<PatchOp>` on `MetadataWritePlan`,
  because its key depends on the output container and a Save keeps it unless
  told otherwise.
- Every cover reader offers its pictures in file order to `CoverChoice` and
  adds no rule of its own, so an export keeps the cover the app shows.
- Thumbnail readers stay allocation-bounded before decode or demux; an unknown
  non-MP4 tag container returns no thumbnail rather than an unbounded demux.
- When metadata policy crosses planning and writing, prove it through the
  production processing planner and read tags from real output files on each
  affected route; a test with supplied metadata cannot prove planner
  forwarding. Also prove source tags stay untouched by processing.
- The tag-interop snapshots
  (`crates/abb-engine/tests/snapshots/tag-interop/`) pin what every
  tag-writing route leaves in a file. Rewrite one with
  `ABB_BLESS_TAG_SNAPSHOTS=1` only for an intended output change, and name
  that change in the commit.
- Keep `set | clear | absent` field semantics (absent keeps the source value)
  across save, processing, naming, write plans, validation, and covers. Clear
  is the explicit blank action, never a sentinel value.
- Publication-date and series sequence validation stay in this module.
- Known bug, low priority (#635): a FLAC Save writes Comment and Description
  under one Vorbis key, because FFmpeg maps `comment` to `DESCRIPTION`
  (`libavformat/vorbiscomment.c`), so they read back joined.
- `----:com.apple.iTunes:SERIES` and `SERIES-PART` are FFmpeg read and clear
  keys, never write keys, so a stray tag from an older ABB still reads and a
  series clear removes it; the mp4ameta sink writes the MP4 mirrors.
- MP4 artist and composer reads join repeated values with `;`, as FFmpeg's
  text projection does. A name list is one text value; nothing preserves its
  structure or a distinct album artist.
- Drop FFmpeg probe and remux contexts before mp4ameta writes the same path or
  the source file is replaced.
- CUE `FILE` text never chooses another audio path and never refuses a sheet,
  because a real rip's label holds colons and Windows paths.
- Chapter repair refuses an end it would move more than 1 s, because real rips
  drift by a few ms and a larger move guesses at the book's structure.
- ABB's own tag write moves a source's fingerprint forward only when the file
  still matched it just before the write, so a change made outside ABB is
  still refused.
- An MP4 is remuxed only for a real chapter change, because FFmpeg's mov
  writer drops the freeform tags ABB does not model.
- Opus Matroska to MP4 copies restore the 48 kHz timeline lost to millisecond
  container rounding; a discontinuity fails rather than discards audio. MP4
  frame-size metadata must let the muxer honor trailing discard padding. An
  MP3 source without trim metadata gets no invented Xing decoder delay.
