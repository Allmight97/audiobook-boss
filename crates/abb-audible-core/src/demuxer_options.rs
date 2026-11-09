use secrecy::{ExposeSecret, SecretString};

use crate::license::AudibleDecryptionMaterial;
use crate::protected_file::ProtectedFileFacts;
use crate::DecryptKind;

const ACTIVATION_BYTES_HEX_LEN: usize = 8;
const AES_128_HEX_LEN: usize = 32;
const AAXC_MAJOR_BRAND: &str = "aaxc";

/// One option for FFmpeg's MP4 demuxer. It carries a secret, so it has no
/// `Debug`.
pub struct DemuxerOption {
    pub name: &'static str,
    pub value: SecretString,
}

/// Names the malformed value, never its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemuxerOptionsError {
    ActivationBytes,
    Key,
    Iv,
}

impl std::fmt::Display for DemuxerOptionsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (value, digits) = match self {
            Self::ActivationBytes => ("AAX activation bytes", ACTIVATION_BYTES_HEX_LEN),
            Self::Key => ("AAXC key", AES_128_HEX_LEN),
            Self::Iv => ("AAXC IV", AES_128_HEX_LEN),
        };
        write!(
            formatter,
            "The {value} are not {digits} hexadecimal digits."
        )
    }
}

impl std::error::Error for DemuxerOptionsError {}

/// The demuxer options that decrypt a file with this material. A value of the
/// wrong shape is refused here: FFmpeg logs a value it cannot set.
pub fn demuxer_options(
    material: &AudibleDecryptionMaterial,
) -> Result<Vec<DemuxerOption>, DemuxerOptionsError> {
    match material {
        AudibleDecryptionMaterial::Aax {
            activation_bytes_hex,
        } => Ok(vec![option(
            "activation_bytes",
            activation_bytes_hex,
            ACTIVATION_BYTES_HEX_LEN,
            DemuxerOptionsError::ActivationBytes,
        )?]),
        AudibleDecryptionMaterial::Aaxc { key_hex, iv_hex } => Ok(vec![
            option(
                "audible_key",
                key_hex,
                AES_128_HEX_LEN,
                DemuxerOptionsError::Key,
            )?,
            option(
                "audible_iv",
                iv_hex,
                AES_128_HEX_LEN,
                DemuxerOptionsError::Iv,
            )?,
        ]),
    }
}

fn option(
    name: &'static str,
    value: &SecretString,
    hex_len: usize,
    error: DemuxerOptionsError,
) -> Result<DemuxerOption, DemuxerOptionsError> {
    let text = value.expose_secret();
    if text.len() == hex_len && text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(DemuxerOption {
            name,
            value: value.clone(),
        })
    } else {
        Err(error)
    }
}

/// Whether the download is the kind of file the license's lane decrypts. FFmpeg
/// applies AAXC keys only to major brand `aaxc` and AAX activation bytes only
/// through an `adrm` box, and otherwise opens the file without decrypting it.
pub fn file_matches_decrypt_kind(facts: &ProtectedFileFacts, kind: DecryptKind) -> bool {
    match kind {
        DecryptKind::Aaxc => facts.major_brand == AAXC_MAJOR_BRAND,
        DecryptKind::Aax => facts.adrm_present,
    }
}

#[cfg(test)]
#[path = "demuxer_options_tests.rs"]
mod tests;
