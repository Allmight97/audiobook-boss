//! Pure Audible provider-protocol logic: license-response decryption, voucher
//! key-material extraction, filename naming, JSON probing, and HTTP download
//! response classification.
//!
//! This crate holds the provider-private logic that needs fast, isolated tests
//! and no Tauri/FFmpeg/IO coupling. The `src-tauri` Audible module is the thin
//! runtime adapter that performs network/file IO and maps these pure results to
//! runtime types. `abb-remote-source-core` stays provider-neutral; Audible
//! specifics live here.

mod download;
mod json_probe;
mod license;
mod naming;

pub use download::{
    classify_download_response, classify_download_response_for_mode, DownloadResponseError,
    ParsedContentRange,
};
pub use json_probe::{find_first_string_for_key, find_first_string_for_keys};
pub use license::{
    audible_decryption_material_from_license, AudibleDecryptionMaterial,
    AudibleLicenseDecryptContext,
};
pub use naming::{
    download_extension_for_strategy, remote_materialized_filename_stem,
    supplemental_pdf_display_file_name, title_ref,
};
