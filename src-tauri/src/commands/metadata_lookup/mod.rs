mod mapping;
mod parse;
mod providers;
mod service;
mod types;

use crate::commands::CommandResult;

pub use types::{
    MetadataLookupDiagnostic, MetadataLookupDiagnosticKind, MetadataLookupResponse, MetadataSource,
    OnlineMetadataResult,
};

#[tauri::command]
#[specta::specta]
pub async fn search_online_metadata(
    query: String,
    sources: Option<Vec<MetadataSource>>,
    limit: Option<u8>,
) -> CommandResult<MetadataLookupResponse> {
    Ok(service::search_online_metadata(query, sources, limit).await?)
}
