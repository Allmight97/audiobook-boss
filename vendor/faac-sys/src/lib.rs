//! Raw bindings to ABB's pinned FAAC encoder. The caller owns each handle,
//! copies library-owned ASC before close, and supplies signed-16-scaled FLOAT PCM.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests;
