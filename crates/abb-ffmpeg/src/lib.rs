//! ABB's safe layer over the FFmpeg C API. It exposes only the operations ABB
//! performs, as owned values and typed errors; no FFmpeg pointer, lifetime,
//! or `unsafe` crosses its public surface.
#![deny(clippy::undocumented_unsafe_blocks, unsafe_op_in_unsafe_fn)]

mod context;
mod error;
mod input;
mod output;
mod packet;
mod text;

pub use context::{Chapter, Disposition, Medium, Rational, StreamFacts};
pub use error::{Error, Result, Step};
pub use input::{Input, OpenedInput};
pub use output::{Muxer, Output};
pub use packet::Packet;
pub use text::{SecretOptions, Tags, Text};
