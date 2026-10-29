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
pub use license_facts::{
    choose_acquisition_strategy, license_facts_from_value, LicenseDenial, LicenseDenialReason,
    LicenseFacts,
};
pub use naming::{
    download_extension_for_strategy, remote_materialized_filename_stem,
    supplemental_pdf_display_file_name, title_ref,
};
