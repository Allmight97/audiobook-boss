mod plan;
mod run;

pub use run::{preflight_payload, process_payload};

pub(crate) use run::{
    resolve_effective_processing_metadata, resolve_naming_metadata, validate_batch_input_path,
};
