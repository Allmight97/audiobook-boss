use crate::strategy::AcquisitionStrategy;
use aes::Aes128;
use base64::{
    engine::general_purpose::{
        STANDARD as BASE64_STANDARD, STANDARD_NO_PAD as BASE64_STANDARD_NO_PAD, URL_SAFE,
        URL_SAFE_NO_PAD,
    },
    Engine as _,
};
use cbc::cipher::{block_padding::NoPadding, BlockModeDecrypt, KeyIvInit};
use secrecy::SecretBox;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::json_probe::find_in_tree;
use crate::reply_keys::{asin, license_response};

type Aes128CbcDec = cbc::Decryptor<Aes128>;

/// Fixed-size secrets that zeroize on drop. The enum has no `Debug`.
pub enum AudibleDecryptionMaterial {
    Aax {
        activation_bytes: SecretBox<[u8; 4]>,
    },
    Aaxc {
        key: SecretBox<[u8; 16]>,
        iv: SecretBox<[u8; 16]>,
    },
}

fn secret<const N: usize>(bytes: [u8; N]) -> SecretBox<[u8; N]> {
    SecretBox::new(Box::new(bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecryptKind {
    Aax,
    Aaxc,
}

impl AudibleDecryptionMaterial {
    pub fn kind(&self) -> DecryptKind {
        match self {
            Self::Aax { .. } => DecryptKind::Aax,
            Self::Aaxc { .. } => DecryptKind::Aaxc,
        }
    }
}

/// Which reader produced the decryption material, for the license log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialSource {
    LicenseResponse,
    Voucher,
    ActivationBytesScan,
    KeyIvPairScan,
}

impl MaterialSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::LicenseResponse => "license_response",
            Self::Voucher => "voucher",
            Self::ActivationBytesScan => "activation_bytes_scan",
            Self::KeyIvPairScan => "key_iv_pair_scan",
        }
    }
}

pub struct LocatedDecryptionMaterial {
    pub material: AudibleDecryptionMaterial,
    pub source: MaterialSource,
}

pub struct AudibleLicenseDecryptContext {
    pub device_type: String,
    pub device_serial: String,
    pub amazon_account_id: String,
}

struct VoucherKeyMaterial {
    key: Vec<u8>,
    iv: Option<Vec<u8>>,
}

/// Extract decryption material for the given strategy from a license response,
/// trying (in order) an encrypted `license_response` blob, an inline voucher,
/// and strategy-specific activation/key-iv scans of the whole reply.
pub fn audible_decryption_material_from_license(
    license: &Value,
    strategy: AcquisitionStrategy,
    title_id: &str,
    decrypt_context: Option<&AudibleLicenseDecryptContext>,
) -> Option<LocatedDecryptionMaterial> {
    let located = |material, source| LocatedDecryptionMaterial { material, source };
    if let Some(material) =
        decrypt_license_response_material(license, strategy, title_id, decrypt_context)
    {
        return Some(located(material, MaterialSource::LicenseResponse));
    }

    let key_material = find_voucher_key_material(license);
    if let Some(material) = audible_decryption_material_from_key_material(key_material.as_ref()) {
        return Some(located(material, MaterialSource::Voucher));
    }

    match strategy {
        AcquisitionStrategy::DownloadThenDecryptAax => {
            find_activation_bytes(license).map(|activation_bytes| {
                located(
                    AudibleDecryptionMaterial::Aax {
                        activation_bytes: secret(activation_bytes),
                    },
                    MaterialSource::ActivationBytesScan,
                )
            })
        }
        AcquisitionStrategy::DownloadThenDecryptAaxc => {
            find_key_iv_pair(license).map(|(key, iv)| {
                located(
                    AudibleDecryptionMaterial::Aaxc {
                        key: secret(key),
                        iv: secret(iv),
                    },
                    MaterialSource::KeyIvPairScan,
                )
            })
        }
        _ => None,
    }
}

fn decrypt_license_response_material(
    license: &Value,
    strategy: AcquisitionStrategy,
    title_id: &str,
    decrypt_context: Option<&AudibleLicenseDecryptContext>,
) -> Option<AudibleDecryptionMaterial> {
    match strategy {
        AcquisitionStrategy::DownloadThenDecryptAax
        | AcquisitionStrategy::DownloadThenDecryptAaxc => {}
        _ => return None,
    }

    let context = decrypt_context?;
    let license_response = license_response(license)?;
    let asin = asin(license).unwrap_or_else(|| title_id.into());
    let decrypted = decrypt_license_response(&license_response, &asin, context)?;
    let key_material = find_voucher_key_material(&decrypted);
    audible_decryption_material_from_key_material(key_material.as_ref())
}

fn decrypt_license_response(
    license_response: &str,
    asin: &str,
    context: &AudibleLicenseDecryptContext,
) -> Option<Value> {
    let mut ciphertext = decode_base64_any(license_response)?;
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return None;
    }

    let key_components = format!(
        "{}{}{}{}",
        context.device_type, context.device_serial, context.amazon_account_id, asin
    );
    let hash = Sha256::digest(key_components.as_bytes());
    let (key, iv) = hash.split_at(16);
    let plaintext = Aes128CbcDec::new_from_slices(key, iv)
        .ok()?
        .decrypt_padded::<NoPadding>(&mut ciphertext)
        .ok()?;
    let json_bytes = plaintext
        .split(|byte| *byte == 0)
        .next()
        .unwrap_or(plaintext);
    serde_json::from_slice(json_bytes).ok()
}

fn audible_decryption_material_from_key_material(
    key_material: Option<&VoucherKeyMaterial>,
) -> Option<AudibleDecryptionMaterial> {
    let material = key_material?;
    match (material.key.as_slice(), material.iv.as_deref()) {
        (activation_bytes, None) => Some(AudibleDecryptionMaterial::Aax {
            activation_bytes: secret(activation_bytes.try_into().ok()?),
        }),
        (key, Some(iv)) => Some(AudibleDecryptionMaterial::Aaxc {
            key: secret(key.try_into().ok()?),
            iv: secret(iv.try_into().ok()?),
        }),
    }
}

fn find_voucher_key_material(value: &Value) -> Option<VoucherKeyMaterial> {
    find_in_tree(value, &mut |map, _| {
        if let Some(voucher) = map
            .get("voucher")
            .or_else(|| map.get("Voucher"))
            .or_else(|| map.get("content_license"))
            .or_else(|| map.get("contentLicense"))
            .and_then(find_voucher_key_material)
        {
            return Some(voucher);
        }
        let key = find_bytes_for_keys_in_object(
            map,
            &[
                "key",
                "Key",
                "keyHex",
                "key_hex",
                "activation_bytes",
                "activationBytes",
                "activationBytesHex",
                "activation_bytes_hex",
            ],
            &[4, 16],
        )?;
        let iv = find_bytes_for_keys_in_object(map, &["iv", "Iv", "ivHex", "iv_hex"], &[16]);
        Some(VoucherKeyMaterial { key, iv })
    })
}

fn find_activation_bytes(value: &Value) -> Option<[u8; 4]> {
    find_in_tree(value, &mut |map, _| {
        let bytes = find_bytes_for_keys_in_object(
            map,
            &[
                "activation_bytes",
                "activationBytes",
                "activationBytesHex",
                "activation_bytes_hex",
                "activation",
                "key",
                "Key",
            ],
            &[4],
        )?;
        bytes.try_into().ok()
    })
}

fn find_key_iv_pair(value: &Value) -> Option<([u8; 16], [u8; 16])> {
    find_in_tree(value, &mut |map, _| {
        let key = find_bytes_for_keys_in_object(map, &["key", "Key", "keyHex", "key_hex"], &[16])?;
        let iv = find_bytes_for_keys_in_object(map, &["iv", "Iv", "ivHex", "iv_hex"], &[16])?;
        Some((key.try_into().ok()?, iv.try_into().ok()?))
    })
}

fn find_bytes_for_keys_in_object(
    map: &serde_json::Map<String, Value>,
    keys: &[&str],
    expected_lens: &[usize],
) -> Option<Vec<u8>> {
    keys.iter()
        .filter_map(|key| map.get(*key))
        .find_map(|value| decode_secret_bytes(value, expected_lens))
}

fn decode_secret_bytes(value: &Value, expected_lens: &[usize]) -> Option<Vec<u8>> {
    match value {
        Value::String(raw) => decode_secret_string(raw, expected_lens),
        Value::Array(values) => {
            let bytes = values
                .iter()
                .map(|value| value.as_u64().and_then(|byte| u8::try_from(byte).ok()))
                .collect::<Option<Vec<_>>>()?;
            expected_lens.contains(&bytes.len()).then_some(bytes)
        }
        _ => None,
    }
}

fn decode_secret_string(raw: &str, expected_lens: &[usize]) -> Option<Vec<u8>> {
    let trimmed = raw.trim();
    let hex = trimmed
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect::<String>();
    if expected_lens
        .iter()
        .any(|expected_len| hex.len() == expected_len * 2)
        && trimmed.chars().all(|character| {
            character.is_ascii_hexdigit()
                || character == '-'
                || character == ':'
                || character == ' '
        })
    {
        return decode_hex(&hex);
    }

    [
        &BASE64_STANDARD,
        &BASE64_STANDARD_NO_PAD,
        &URL_SAFE,
        &URL_SAFE_NO_PAD,
    ]
    .iter()
    .find_map(|engine| {
        engine
            .decode(trimmed)
            .ok()
            .filter(|bytes| expected_lens.contains(&bytes.len()))
    })
}

fn decode_base64_any(raw: &str) -> Option<Vec<u8>> {
    let trimmed = raw.trim();
    [
        &BASE64_STANDARD,
        &BASE64_STANDARD_NO_PAD,
        &URL_SAFE,
        &URL_SAFE_NO_PAD,
    ]
    .iter()
    .find_map(|engine| engine.decode(trimmed).ok())
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).ok())
        .collect()
}

#[cfg(test)]
#[path = "license_tests.rs"]
mod tests;
