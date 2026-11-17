use secrecy::{ExposeSecret, SecretString};

use crate::license::AudibleDecryptionMaterial;
use crate::protected_file::ProtectedFileFacts;
use crate::DecryptKind;

const AAXC_MAJOR_BRAND: &str = "aaxc";

pub struct DemuxerOption {
    pub name: &'static str,
    pub value: SecretString,
}

pub fn demuxer_options(material: &AudibleDecryptionMaterial) -> Vec<DemuxerOption> {
    match material {
        AudibleDecryptionMaterial::Aax { activation_bytes } => {
            vec![option("activation_bytes", activation_bytes.expose_secret())]
        }
        AudibleDecryptionMaterial::Aaxc { key, iv } => vec![
            option("audible_key", key.expose_secret()),
            option("audible_iv", iv.expose_secret()),
        ],
    }
}

fn option(name: &'static str, bytes: &[u8]) -> DemuxerOption {
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    DemuxerOption {
        name,
        value: SecretString::from(hex),
    }
}

pub fn file_matches_decrypt_kind(facts: &ProtectedFileFacts, kind: DecryptKind) -> bool {
    match kind {
        DecryptKind::Aaxc => facts.major_brand == AAXC_MAJOR_BRAND,
        DecryptKind::Aax => facts.drm_boxes.is_some_and(|boxes| boxes.adrm_present),
    }
}

#[cfg(test)]
#[path = "demuxer_options_tests.rs"]
mod tests;
