use abb_remote_source_core::AcquisitionStrategy;
use aes::Aes128;
use base64::{
    engine::general_purpose::{
        STANDARD as BASE64_STANDARD, STANDARD_NO_PAD as BASE64_STANDARD_NO_PAD, URL_SAFE,
        URL_SAFE_NO_PAD,
    },
    Engine as _,
};
use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use secrecy::SecretString;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::json_probe::find_first_string_for_keys;

type Aes128CbcDec = cbc::Decryptor<Aes128>;

/// Decryption material extracted from an Audible license response, ready to hand
/// to the AAXClean materializer. Secrets stay wrapped in [`SecretString`].
pub enum AudibleDecryptionMaterial {
    Aax {
        activation_bytes_hex: SecretString,
    },
    Aaxc {
        key_hex: SecretString,
        iv_hex: SecretString,
    },
}

/// Inputs required to decrypt an Audible `license_response` blob. Built by the
/// runtime adapter from stored auth.
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
        .decrypt_padded_mut::<NoPadding>(&mut ciphertext)
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
mod tests {
    use super::*;
    use cbc::cipher::BlockEncryptMut;
    use secrecy::ExposeSecret;
    use serde_json::json;

    type Aes128CbcEnc = cbc::Encryptor<Aes128>;

    fn fixture_decrypt_context() -> AudibleLicenseDecryptContext {
        AudibleLicenseDecryptContext {
            device_type: "A2CZJZGLK2JJVM".to_string(),
            device_serial: "device-serial".to_string(),
            amazon_account_id: "account-1".to_string(),
        }
    }

    fn encrypted_license_response(
        voucher: Value,
        asin: &str,
        context: &AudibleLicenseDecryptContext,
    ) -> String {
        let mut plaintext = serde_json::to_vec(&voucher).expect("voucher json");
        let padded_len = plaintext.len().next_multiple_of(16);
        plaintext.resize(padded_len, 0);
        let key_components = format!(
            "{}{}{}{}",
            context.device_type, context.device_serial, context.amazon_account_id, asin
        );
        let hash = Sha256::digest(key_components.as_bytes());
        let (key, iv) = hash.split_at(16);
        let ciphertext = Aes128CbcEnc::new_from_slices(key, iv)
            .expect("cipher")
            .encrypt_padded_mut::<NoPadding>(&mut plaintext, padded_len)
            .expect("encrypt");
        BASE64_STANDARD.encode(ciphertext)
    }

    #[test]
    fn aax_license_response_decrypts_adrm_voucher_activation_bytes() {
        let context = fixture_decrypt_context();
        let asin = "B000000001";
        let response = json!({
            "content_license": {
                "asin": asin,
                "content_url": "https://cdn.example.test/book.aax",
                "drm_type": "Adrm",
                "license_response": encrypted_license_response(
                    json!({ "key": [10, 27, 44, 61] }),
                    asin,
                    &context
                )
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAax,
            asin,
            Some(&context),
        )
        .expect("aax material");

        match material {
            AudibleDecryptionMaterial::Aax {
                activation_bytes_hex,
            } => assert_eq!(activation_bytes_hex.expose_secret(), "0a1b2c3d"),
            AudibleDecryptionMaterial::Aaxc { .. } => panic!("unexpected aaxc material"),
        }
    }

    #[test]
    fn aaxc_license_response_decrypts_adrm_voucher_key_iv() {
        let context = fixture_decrypt_context();
        let asin = "B000000002";
        let response = json!({
            "content_license": {
                "Asin": asin,
                "content_url": "https://cdn.example.test/book.aaxc",
                "drm_type": "Adrm",
                "licenseResponse": encrypted_license_response(
                    json!({
                        "key": "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d",
                        "iv": BASE64_STANDARD.encode([
                            0x2e, 0x2f, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
                            0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f, 0x5a, 0x5b
                        ])
                    }),
                    asin,
                    &context
                )
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAaxc,
            asin,
            Some(&context),
        )
        .expect("aaxc material");

        match material {
            AudibleDecryptionMaterial::Aaxc { key_hex, iv_hex } => {
                assert_eq!(key_hex.expose_secret(), "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d");
                assert_eq!(iv_hex.expose_secret(), "2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b");
            }
            AudibleDecryptionMaterial::Aax { .. } => panic!("unexpected aax material"),
        }
    }

    #[test]
    fn aax_license_material_extracts_activation_bytes_without_public_surface() {
        let response = json!({
            "content_license": {
                "content_url": "https://cdn.example.test/book.aax",
                "drm_type": "Mpeg",
                "voucher": {
                    "key": [10, 27, 44, 61]
                }
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAax,
            "B000000001",
            None,
        )
        .expect("aax material");

        match material {
            AudibleDecryptionMaterial::Aax {
                activation_bytes_hex,
            } => assert_eq!(activation_bytes_hex.expose_secret(), "0a1b2c3d"),
            AudibleDecryptionMaterial::Aaxc { .. } => panic!("unexpected aaxc material"),
        }
    }

    #[test]
    fn aax_license_material_extracts_url_safe_unpadded_voucher_key() {
        let response = json!({
            "content_license": {
                "content_url": "https://cdn.example.test/book.aax",
                "drm_type": "Mpeg",
                "voucher": {
                    "Key": URL_SAFE_NO_PAD.encode([0xfb, 0xff, 0xee, 0xdd])
                }
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAax,
            "B000000001",
            None,
        )
        .expect("aax material");

        match material {
            AudibleDecryptionMaterial::Aax {
                activation_bytes_hex,
            } => assert_eq!(activation_bytes_hex.expose_secret(), "fbffeedd"),
            AudibleDecryptionMaterial::Aaxc { .. } => panic!("unexpected aaxc material"),
        }
    }

    #[test]
    fn aaxc_license_material_extracts_key_and_iv_from_hex_or_base64() {
        let response = json!({
            "content_license": {
                "content_url": "https://cdn.example.test/book.aaxc",
                "drm_type": "Mpeg",
                "voucher": {
                    "key": "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d",
                    "iv": BASE64_STANDARD.encode([
                        0x2e, 0x2f, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
                        0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f, 0x5a, 0x5b
                    ])
                }
            }
        });

        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAaxc,
            "B000000001",
            None,
        )
        .expect("aaxc material");

        match material {
            AudibleDecryptionMaterial::Aaxc { key_hex, iv_hex } => {
                assert_eq!(key_hex.expose_secret(), "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d");
                assert_eq!(iv_hex.expose_secret(), "2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b");
            }
            AudibleDecryptionMaterial::Aax { .. } => panic!("unexpected aax material"),
        }
    }
}
