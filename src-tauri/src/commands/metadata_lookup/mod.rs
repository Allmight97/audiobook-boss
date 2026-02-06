mod mapping;
mod parse;
mod providers;
mod service;
mod types;

use crate::errors::Result;

pub use types::{MetadataSource, OnlineMetadataResult};

#[tauri::command]
pub async fn search_online_metadata(
    query: String,
    sources: Option<Vec<MetadataSource>>,
    limit: Option<u8>,
) -> Result<Vec<OnlineMetadataResult>> {
    service::search_online_metadata(query, sources, limit).await
}
