//! Raw bindings to ABB's bundled FAAC encoder and FAAD decoder. The caller owns
//! each handle, copies library-owned ASC before close, supplies signed-16-scaled
//! FLOAT PCM to the encoder, and receives -1..1 FLOAT PCM from the decoder.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod faad_tests;
#[cfg(test)]
mod tests;
