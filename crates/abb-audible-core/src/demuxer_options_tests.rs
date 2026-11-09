use super::*;
use secrecy::ExposeSecret;

const KEY: &str = "000102030405060708090a0b0c0d0e0f";
const IV: &str = "f0e0d0c0b0a090807060504030201000";

fn aax(activation: &str) -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aax {
        activation_bytes_hex: SecretString::from(activation),
    }
}

fn aaxc(key: &str, iv: &str) -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aaxc {
        key_hex: SecretString::from(key),
        iv_hex: SecretString::from(iv),
    }
}

fn facts(major_brand: &str, adrm_present: bool) -> ProtectedFileFacts {
    ProtectedFileFacts {
        major_brand: major_brand.to_string(),
        compatible_brands: Vec::new(),
        adrm_present,
        aavd_entry_present: true,
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
    let options = demuxer_options(&aax("1CEB00da")).expect("valid activation bytes");

    assert_eq!(pairs(&options), [("activation_bytes", "1CEB00da")]);
}

#[test]
fn aaxc_key_and_iv_become_the_audible_key_and_iv_options() {
    let options = demuxer_options(&aaxc(KEY, IV)).expect("valid key and iv");

    assert_eq!(pairs(&options), [("audible_key", KEY), ("audible_iv", IV)]);
}

#[test]
fn a_malformed_value_is_refused_by_name_and_the_message_never_holds_it() {
    let short_key = &KEY[..30];
    let non_hex_iv = format!("zz{}", &IV[2..]);
    let non_hex_iv = non_hex_iv.as_str();
    let cases = [
        (
            aax("1ceb00"),
            DemuxerOptionsError::ActivationBytes,
            "1ceb00",
        ),
        (
            aax("1ceb00dg"),
            DemuxerOptionsError::ActivationBytes,
            "1ceb00dg",
        ),
        (
            aax("1ceb00da00"),
            DemuxerOptionsError::ActivationBytes,
            "1ceb00da00",
        ),
        (aaxc(short_key, IV), DemuxerOptionsError::Key, short_key),
        (aaxc(KEY, non_hex_iv), DemuxerOptionsError::Iv, non_hex_iv),
        (aaxc("", IV), DemuxerOptionsError::Key, IV),
    ];
    for (material, expected, value) in cases {
        let error = demuxer_options(&material).err().expect("malformed value");

        assert_eq!(error, expected);
        assert!(!error.to_string().contains(value), "{error}");
        assert!(!error.to_string().contains(KEY), "{error}");
    }
}

#[test]
fn an_aaxc_download_matches_only_the_aaxc_lane() {
    assert!(file_matches_decrypt_kind(
        &facts("aaxc", false),
        DecryptKind::Aaxc
    ));
    assert!(!file_matches_decrypt_kind(
        &facts("aax ", true),
        DecryptKind::Aaxc
    ));
    assert!(!file_matches_decrypt_kind(
        &facts("M4B ", false),
        DecryptKind::Aaxc
    ));
}

#[test]
fn an_aax_download_matches_the_aax_lane_only_with_an_adrm_box() {
    assert!(file_matches_decrypt_kind(
        &facts("aax ", true),
        DecryptKind::Aax
    ));
    assert!(!file_matches_decrypt_kind(
        &facts("aaxc", false),
        DecryptKind::Aax
    ));
}
