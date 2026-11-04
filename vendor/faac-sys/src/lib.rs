//! Raw bindings to ABB's bundled FAAC encoder and FAAD3 decoder. The caller owns
//! each handle, copies library-owned ASC before close, and exchanges FLOAT PCM
//! at unity full scale with both libraries.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod faad_tests;
#[cfg(test)]
mod tests;
