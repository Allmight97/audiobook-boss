mod download;
mod json_probe;
mod library_item;
mod license;
mod license_facts;
mod naming;
mod reply_keys;
mod strategy;
mod title_metadata;

pub use download::{classify_download_response, DownloadResponseError, ParsedContentRange};
pub use json_probe::{find_first_string_for_key, OwnedPathStep};
pub use library_item::{library_items_from_page, LibraryItem};
pub use license::{
    audible_decryption_material_from_license, AudibleDecryptionMaterial,
    AudibleLicenseDecryptContext, DecryptKind, LocatedDecryptionMaterial, MaterialSource,
};
pub use license_facts::{
    choose_acquisition_strategy, license_facts_from_value, LicenseDenial, LicenseDenialReason,
    LicenseFacts,
};
pub use naming::{
    remote_materialized_filename_stem, supplemental_pdf_display_file_name, title_ref,
};
pub use strategy::AcquisitionStrategy;
pub use title_metadata::{title_metadata_from_value, TitleMetadata};
