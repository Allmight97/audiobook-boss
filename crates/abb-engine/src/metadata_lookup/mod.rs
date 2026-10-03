//! Online metadata lookup across Audible, Audnexus, and Open Library.

mod mapping;
mod parse;
mod providers;
mod service;
mod types;

pub(crate) use service::search_online_metadata;
pub use types::{
    MetadataLookupDiagnostic, MetadataLookupDiagnosticKind, MetadataLookupResponse, MetadataSource,
    OnlineMetadataResult,
};
