use abb_remote_source_core::AcquisitionStrategy;
use aes::Aes128;
use base64::{
    engine::general_purpose::{
        STANDARD as BASE64_STANDARD, STANDARD_NO_PAD as BASE64_STANDARD_NO_PAD, URL_SAFE,
        URL_SAFE_NO_PAD,
    },
    Engine as _,
};
use cbc::cipher::{block_padding::NoPadding, BlockModeDecrypt, KeyIvInit};
use secrecy::SecretString;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::json_probe::find_first_string_for_keys;

type Aes128CbcDec = cbc::Decryptor<Aes128>;

pub enum AudibleDecryptionMaterial {
    Aax {
        activation_bytes_hex: SecretString,
    },
    Aaxc {
        key_hex: SecretString,
        iv_hex: SecretString,
    },
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
/// and strategy-specific activation/key-iv fallbacks.
pub fn audible_decryption_material_from_license(
    license: &Value,
    strategy: AcquisitionStrategy,
    title_id: &str,
    decrypt_context: Option<&AudibleLicenseDecryptContext>,
) -> Option<AudibleDecryptionMaterial> {
    if let Some(material) =
        decrypt_license_response_material(license, strategy, title_id, decrypt_context)
    {
        return Some(material);
    }

    let key_material = find_voucher_key_material(license);
    if let Some(material) = audible_decryption_material_from_key_material(key_material.as_ref()) {
        return Some(material);
    }

    match strategy {
        AcquisitionStrategy::DownloadThenDecryptAax => {
            find_activation_bytes(license).map(|activation_bytes| AudibleDecryptionMaterial::Aax {
                activation_bytes_hex: SecretString::from(hex_encode(&activation_bytes)),
            })
        }
        AcquisitionStrategy::DownloadThenDecryptAaxc => {
            find_key_iv_pair(license).map(|(key, iv)| AudibleDecryptionMaterial::Aaxc {
                key_hex: SecretString::from(hex_encode(&key)),
                iv_hex: SecretString::from(hex_encode(&iv)),
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
    let license_response =
        find_first_string_for_keys(license, &["license_response", "licenseResponse"])?;
    let asin =
        find_first_string_for_keys(license, &["asin", "Asin"]).unwrap_or_else(|| title_id.into());
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
        (activation_bytes, None) if activation_bytes.len() == 4 => {
            Some(AudibleDecryptionMaterial::Aax {
                activation_bytes_hex: SecretString::from(hex_encode(activation_bytes)),
            })
        }
        (key, Some(iv)) if key.len() == 16 && iv.len() == 16 => {
            Some(AudibleDecryptionMaterial::Aaxc {
                key_hex: SecretString::from(hex_encode(key)),
                iv_hex: SecretString::from(hex_encode(iv)),
            })
        }
        _ => None,
    }
}

fn find_voucher_key_material(value: &Value) -> Option<VoucherKeyMaterial> {
    match value {
        Value::Object(map) => {
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
            );
            if let Some(key) = key {
                let iv =
                    find_bytes_for_keys_in_object(map, &["iv", "Iv", "ivHex", "iv_hex"], &[16]);
                return Some(VoucherKeyMaterial { key, iv });
            }
            map.values().find_map(find_voucher_key_material)
        }
        Value::Array(values) => values.iter().find_map(find_voucher_key_material),
        _ => None,
    }
}

fn find_activation_bytes(value: &Value) -> Option<Vec<u8>> {
    find_bytes_for_keys(
        value,
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
    )
}

fn find_key_iv_pair(value: &Value) -> Option<(Vec<u8>, Vec<u8>)> {
    match value {
        Value::Object(map) => {
            let key =
                find_bytes_for_keys_in_object(map, &["key", "Key", "keyHex", "key_hex"], &[16]);
            let iv = find_bytes_for_keys_in_object(map, &["iv", "Iv", "ivHex", "iv_hex"], &[16]);
            if let (Some(key), Some(iv)) = (key, iv) {
                return Some((key, iv));
            }
            map.values().find_map(find_key_iv_pair)
        }
        Value::Array(values) => values.iter().find_map(find_key_iv_pair),
        _ => None,
    }
}

fn find_bytes_for_keys(value: &Value, keys: &[&str], expected_lens: &[usize]) -> Option<Vec<u8>> {
    match value {
        Value::Object(map) => {
            if let Some(bytes) = find_bytes_for_keys_in_object(map, keys, expected_lens) {
                return Some(bytes);
            }
            map.values()
                .find_map(|entry| find_bytes_for_keys(entry, keys, expected_lens))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_bytes_for_keys(entry, keys, expected_lens)),
        _ => None,
    }
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

fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

#[cfg(test)]
#[path = "license_tests.rs"]
mod tests;
