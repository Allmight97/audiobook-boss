use super::*;
use crate::protected_file::DrmBoxes;
use secrecy::SecretBox;

fn aax(activation: [u8; 4]) -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aax {
        activation_bytes: SecretBox::new(Box::new(activation)),
    }
}

fn aaxc(key: [u8; 16], iv: [u8; 16]) -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aaxc {
        key: SecretBox::new(Box::new(key)),
        iv: SecretBox::new(Box::new(iv)),
    }
}

fn facts(major_brand: &str, adrm_present: Option<bool>) -> ProtectedFileFacts {
    ProtectedFileFacts {
        major_brand: major_brand.to_string(),
        compatible_brands: Vec::new(),
        drm_boxes: adrm_present.map(|adrm_present| DrmBoxes {
            adrm_present,
            aavd_entry_present: true,
        }),
    }
}

fn pairs(options: &[DemuxerOption]) -> Vec<(&'static str, &str)> {
    options
        .iter()
        .map(|option| (option.name, option.value.expose_secret()))
        .collect()
}

#[test]
fn aax_activation_bytes_become_the_activation_bytes_option() {
    let options = demuxer_options(&aax([0x1c, 0xeb, 0x00, 0xda]));

    assert_eq!(pairs(&options), [("activation_bytes", "1ceb00da")]);
}

#[test]
fn aaxc_key_and_iv_become_the_audible_key_and_iv_options() {
    let key = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f,
    ];
    let iv = [0xf0; 16];

    let options = demuxer_options(&aaxc(key, iv));

    assert_eq!(
        pairs(&options),
        [
            ("audible_key", "000102030405060708090a0b0c0d0e0f"),
            ("audible_iv", "f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0"),
        ]
    );
}

#[test]
fn an_aaxc_download_matches_only_the_aaxc_lane() {
    assert!(file_matches_decrypt_kind(
        &facts("aaxc", Some(false)),
        DecryptKind::Aaxc
    ));
    assert!(!file_matches_decrypt_kind(
        &facts("aax ", Some(true)),
        DecryptKind::Aaxc
    ));
    assert!(!file_matches_decrypt_kind(
        &facts("M4B ", Some(false)),
        DecryptKind::Aaxc
    ));
}

#[test]
fn an_aax_download_matches_the_aax_lane_only_with_an_adrm_box() {
    assert!(file_matches_decrypt_kind(
        &facts("aax ", Some(true)),
        DecryptKind::Aax
    ));
    assert!(!file_matches_decrypt_kind(
        &facts("aaxc", Some(false)),
        DecryptKind::Aax
    ));
}

#[test]
fn an_aax_download_with_unreadable_sample_descriptions_does_not_match() {
    assert!(!file_matches_decrypt_kind(
        &facts("aax ", None),
        DecryptKind::Aax
    ));
    assert!(file_matches_decrypt_kind(
        &facts("aaxc", None),
        DecryptKind::Aaxc
    ));
}
