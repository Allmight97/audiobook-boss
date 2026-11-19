# Audible core

## Outside facts

- FFmpeg's MP4 demuxer applies AAXC keys only to major brand `aaxc` and AAX
  activation bytes only through an `adrm` box in a sample description
  (`stsd`); any other file opens undecrypted (`protected_file.rs`,
  `demuxer_options.rs`).
- A title lookup reply holds the book's own product under `item` or
  `product`; `title_metadata.rs` reads only that, so a series entry's `title`
  never names the book.
