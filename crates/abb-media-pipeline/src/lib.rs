pub mod audio;
pub mod diagnostics;
pub mod errors;
pub mod execution;
pub mod file_replace;
pub mod metadata;
pub mod natural_order;
pub mod output_artifact;
pub mod owned_dir;
pub mod power;
pub mod processing;

pub use errors::{AppError, Result};

pub type ManagedJobRegistry = std::sync::Arc<processing::JobRegistry>;

#[cfg(test)]
mod test_cases;
