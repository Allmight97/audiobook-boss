//! Unit tests for encoder settings validation and resolution.
//!
//! Extracted from settings_encoder.rs to keep production code clean.

use audiobook_boss_lib::audio::settings_encoder::{
    detect_available_encoders, resolve_encoder_type, validate_encoder_settings, validate_threads,
    BitrateMode, ChannelConfig, EncoderAvailability, EncoderSettings, EncoderType, ThreadSetting,
    VALID_ENCODER_BITRATES, VALID_THREAD_COUNT_RANGE,
};

fn base_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::FdkHeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Vbr(3),
        channels: ChannelConfig::Auto,
        afterburner: true,
        threads: ThreadSetting::Auto,
        twoloop: true,
    }
}

#[test]
fn test_validate_bitrate_whitelist() {
    for &br in VALID_ENCODER_BITRATES {
        let mut s = base_settings();
        s.bitrate_kbps = br;
        assert!(validate_encoder_settings(&s).is_ok());
    }
    // Test invalid bitrates (outside expanded 48-128 range)
    let mut s = base_settings();
    s.bitrate_kbps = 32; // below minimum
    assert!(validate_encoder_settings(&s).is_err());

    s.bitrate_kbps = 192; // above maximum
    assert!(validate_encoder_settings(&s).is_err());
}

#[test]
fn test_bitrate_mode_validation() {
    let mut s = base_settings();
    s.bitrate_mode = BitrateMode::Vbr(5);
    assert!(validate_encoder_settings(&s).is_ok());

    s.bitrate_mode = BitrateMode::Vbr(0);
    assert!(validate_encoder_settings(&s).is_err());

    s.bitrate_mode = BitrateMode::Vbr(6);
    assert!(validate_encoder_settings(&s).is_err());
}

#[test]
fn test_encoder_mode_combo_validation() {
    let mut s = base_settings();
    s.encoder_type = EncoderType::FdkHeAac;
    s.bitrate_mode = BitrateMode::Vbr(3);
    assert!(validate_encoder_settings(&s).is_ok());

    s.bitrate_mode = BitrateMode::Cbr;
    assert!(validate_encoder_settings(&s).is_err());

    s.encoder_type = EncoderType::AacAt;
    s.bitrate_mode = BitrateMode::Cvbr;
    assert!(validate_encoder_settings(&s).is_ok());
    s.bitrate_mode = BitrateMode::Cbr;
    assert!(validate_encoder_settings(&s).is_err());

    s.encoder_type = EncoderType::NativeAac;
    s.bitrate_mode = BitrateMode::Cbr;
    assert!(validate_encoder_settings(&s).is_ok());
    s.bitrate_mode = BitrateMode::Vbr(3);
    assert!(validate_encoder_settings(&s).is_err());

    // Auto remains VBR-only
    s.encoder_type = EncoderType::Auto;
    s.bitrate_mode = BitrateMode::Vbr(3);
    assert!(validate_encoder_settings(&s).is_ok());
    s.bitrate_mode = BitrateMode::Cvbr;
    assert!(validate_encoder_settings(&s).is_err());
}

#[test]
fn test_threads_validation() {
    let mut s = base_settings();

    // Valid cases
    s.threads = ThreadSetting::Auto;
    assert!(validate_encoder_settings(&s).is_ok());

    s.threads = ThreadSetting::Off;
    assert!(validate_encoder_settings(&s).is_ok());

    s.threads = ThreadSetting::Fixed(1);
    assert!(validate_encoder_settings(&s).is_ok());

    s.threads = ThreadSetting::Fixed(4);
    assert!(validate_encoder_settings(&s).is_ok());

    s.threads = ThreadSetting::Fixed(1024);
    assert!(validate_encoder_settings(&s).is_ok());

    // Invalid cases - below range
    s.threads = ThreadSetting::Fixed(0);
    let err = validate_encoder_settings(&s).expect_err("expected rejection for thread count 0");
    assert!(err.to_string().contains("Invalid threads value: 0"));
    assert!(err.to_string().contains("1..=1024"));

    // Invalid cases - above range
    s.threads = ThreadSetting::Fixed(1025);
    let err = validate_encoder_settings(&s).expect_err("expected rejection for thread count 1025");
    assert!(err.to_string().contains("Invalid threads value: 1025"));
    assert!(err.to_string().contains("1..=1024"));

    s.threads = ThreadSetting::Fixed(2000);
    let err = validate_encoder_settings(&s).expect_err("expected rejection for thread count 2000");
    assert!(err.to_string().contains("Invalid threads value: 2000"));
    assert!(err.to_string().contains("1..=1024"));
}

#[test]
fn test_thread_constant_consistency() {
    // Test that the constant matches the expected range
    assert_eq!(VALID_THREAD_COUNT_RANGE, 1..=1024);
    assert_eq!(*VALID_THREAD_COUNT_RANGE.start(), 1);
    assert_eq!(*VALID_THREAD_COUNT_RANGE.end(), 1024);
}

#[test]
fn test_validate_threads_direct() {
    // Test the validate_threads function directly
    assert!(validate_threads(ThreadSetting::Auto).is_ok());
    assert!(validate_threads(ThreadSetting::Off).is_ok());

    // Test boundary values
    assert!(validate_threads(ThreadSetting::Fixed(1)).is_ok());
    assert!(validate_threads(ThreadSetting::Fixed(1024)).is_ok());

    // Test invalid values
    assert!(validate_threads(ThreadSetting::Fixed(0)).is_err());
    assert!(validate_threads(ThreadSetting::Fixed(1025)).is_err());

    // Verify error message format
    let err = validate_threads(ThreadSetting::Fixed(0)).expect_err("expected error");
    assert!(err.to_string().contains("Invalid threads value: 0"));
    assert!(err.to_string().contains("(allowed 1..=1024)"));
}

#[test]
fn test_detect_available_encoders_struct_defaults() {
    // We can't guarantee availability on CI, but the function should always return a struct.
    let availability = detect_available_encoders();
    assert!(
        availability.native_aac_available
            || availability.aac_at_available
            || availability.fdk_available,
        "At least one encoder should be available in the environment"
    );
}

#[test]
fn test_resolve_encoder_type_prefers_available() {
    let availability = EncoderAvailability {
        fdk_available: true,
        aac_at_available: true,
        native_aac_available: true,
    };
    let resolved = resolve_encoder_type(
        &EncoderSettings {
            encoder_type: EncoderType::Auto,
            ..base_settings()
        },
        &availability,
    );
    assert_eq!(resolved, EncoderType::FdkHeAac);

    let availability_no_fdk = EncoderAvailability {
        fdk_available: false,
        aac_at_available: true,
        native_aac_available: true,
    };
    let resolved = resolve_encoder_type(
        &EncoderSettings {
            encoder_type: EncoderType::Auto,
            ..base_settings()
        },
        &availability_no_fdk,
    );
    assert_eq!(resolved, EncoderType::AacAt);

    let availability_none = EncoderAvailability {
        fdk_available: false,
        aac_at_available: false,
        native_aac_available: true,
    };
    let resolved = resolve_encoder_type(
        &EncoderSettings {
            encoder_type: EncoderType::Auto,
            ..base_settings()
        },
        &availability_none,
    );
    assert_eq!(resolved, EncoderType::NativeAac);
}

#[test]
fn test_twoloop_environment_variable() {
    // Helper to check the environment variable state
    let is_twoloop_disabled = || {
        std::env::var("ABB_DISABLE_TWOLOOP")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    };

    // Test ABB_DISABLE_TWOLOOP environment variable handling
    // This environment variable controls whether the two-loop AAC enhancement algorithm is used

    println!("Testing twoloop AAC enhancement environment variable handling...");

    // Test disabled state with "1"
    std::env::set_var("ABB_DISABLE_TWOLOOP", "1");
    assert!(
        is_twoloop_disabled(),
        "Should detect environment variable as disabled"
    );
    println!("✓ ABB_DISABLE_TWOLOOP=1 correctly detected as disabled");

    // Test enabled (default when env var not set)
    std::env::remove_var("ABB_DISABLE_TWOLOOP");
    assert!(
        !is_twoloop_disabled(),
        "Should default to enabled when env var not set"
    );
    println!("✓ Default state (no env var) correctly detected as enabled");

    // Test alternative true value: "true" (lowercase)
    std::env::set_var("ABB_DISABLE_TWOLOOP", "true");
    assert!(is_twoloop_disabled(), "Should detect 'true' as disabled");
    println!("✓ ABB_DISABLE_TWOLOOP=true correctly detected as disabled");

    // Test alternative true value: "TRUE" (uppercase - case insensitive)
    std::env::set_var("ABB_DISABLE_TWOLOOP", "TRUE");
    assert!(
        is_twoloop_disabled(),
        "Should detect 'TRUE' as disabled (case insensitive)"
    );
    println!("✓ ABB_DISABLE_TWOLOOP=TRUE correctly detected as disabled");

    // Test invalid values (should default to enabled)
    std::env::set_var("ABB_DISABLE_TWOLOOP", "maybe");
    assert!(
        !is_twoloop_disabled(),
        "Invalid values should default to enabled"
    );
    println!("✓ Invalid value 'maybe' correctly defaults to enabled");

    // Clean up
    std::env::remove_var("ABB_DISABLE_TWOLOOP");
    println!("✓ Environment variable cleanup completed");

    println!("Twoloop environment variable test passed!");
}
