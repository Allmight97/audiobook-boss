use super::*;
use cbc::cipher::BlockModeEncrypt;
use secrecy::ExposeSecret;
use serde_json::json;

type Aes128CbcEnc = cbc::Encryptor<Aes128>;

fn bytes<const N: usize>(hex: &str) -> [u8; N] {
    std::array::from_fn(|index| {
        u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("hex")
    })
}

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
        .encrypt_padded::<NoPadding>(&mut plaintext, padded_len)
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
    assert_eq!(material.source, MaterialSource::LicenseResponse);
    let material = material.material;

    assert_eq!(material.kind(), DecryptKind::Aax);
    match material {
        AudibleDecryptionMaterial::Aax { activation_bytes } => {
            assert_eq!(activation_bytes.expose_secret(), &bytes::<4>("0a1b2c3d"))
        }
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
    .expect("aaxc material")
    .material;

    assert_eq!(material.kind(), DecryptKind::Aaxc);
    match material {
        AudibleDecryptionMaterial::Aaxc { key, iv } => {
            assert_eq!(
                key.expose_secret(),
                &bytes::<16>("0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d")
            );
            assert_eq!(
                iv.expose_secret(),
                &bytes::<16>("2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b")
            );
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
    .expect("aax material")
    .material;

    match material {
        AudibleDecryptionMaterial::Aax { activation_bytes } => {
            assert_eq!(activation_bytes.expose_secret(), &bytes::<4>("0a1b2c3d"))
        }
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
    .expect("aax material")
    .material;

    match material {
        AudibleDecryptionMaterial::Aax { activation_bytes } => {
            assert_eq!(activation_bytes.expose_secret(), &bytes::<4>("fbffeedd"))
        }
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
    .expect("aaxc material")
    .material;

    match material {
        AudibleDecryptionMaterial::Aaxc { key, iv } => {
            assert_eq!(
                key.expose_secret(),
                &bytes::<16>("0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d")
            );
            assert_eq!(
                iv.expose_secret(),
                &bytes::<16>("2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b")
            );
        }
        AudibleDecryptionMaterial::Aax { .. } => panic!("unexpected aax material"),
    }
}

#[test]
fn the_material_source_names_the_reader_that_found_the_key() {
    let located = |response: Value, strategy| {
        audible_decryption_material_from_license(&response, strategy, "B000000001", None)
            .expect("material")
            .source
    };
    let voucher = located(
        json!({"content_license": {"voucher": {"key": [10, 27, 44, 61]}}}),
        AcquisitionStrategy::DownloadThenDecryptAax,
    );
    assert_eq!(voucher, MaterialSource::Voucher);
    let activation = located(
        json!({"outer": {"activation": [10, 27, 44, 61]}}),
        AcquisitionStrategy::DownloadThenDecryptAax,
    );
    assert_eq!(activation, MaterialSource::ActivationBytesScan);
    let key_iv = located(
        json!({
            "first": {"key": "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d"},
            "second": {
                "key": "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d",
                "iv": "2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b"
            }
        }),
        AcquisitionStrategy::DownloadThenDecryptAaxc,
    );
    assert_eq!(key_iv, MaterialSource::KeyIvPairScan);
    assert_eq!(MaterialSource::KeyIvPairScan.label(), "key_iv_pair_scan");
}
