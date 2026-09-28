//! Pure Audible provider-protocol logic: license-response interpretation and
//! acquisition strategy, license decryption, voucher key-material extraction,
//! filename naming, JSON probing, and HTTP download response classification.
//!
//! This crate holds the provider-private logic that needs fast, isolated tests
//! and no Tauri/FFmpeg/IO coupling. The `src-tauri` Audible module is the thin
//! runtime adapter that performs network/file IO and maps these pure results to
//! runtime types. `abb-remote-source-core` stays provider-neutral; Audible
//! specifics live here.

mod download;
mod json_probe;
mod license;
mod license_facts;
mod naming;

pub use download::{classify_download_response, DownloadResponseError, ParsedContentRange};
pub use json_probe::{find_first_string_for_key, find_first_string_for_keys};
pub use license::{
    audible_decryption_material_from_license, AudibleDecryptionMaterial,
    AudibleLicenseDecryptContext,
};
pub use license_facts::{choose_acquisition_strategy, license_facts_from_value, LicenseFacts};
pub use naming::{
    download_extension_for_strategy, remote_materialized_filename_stem,
    supplemental_pdf_display_file_name, title_ref,
};
