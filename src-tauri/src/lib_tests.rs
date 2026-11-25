use super::{
    monitor_fit_window_size, refuses_production_identity, window_title, PRODUCTION_IDENTIFIER,
};

#[test]
fn monitor_fit_window_size_fits_each_monitor() {
    let none = None;
    let full = Some((1600.0, 1000.0));
    let rows = [
        (0, 1080, 1.0, none),
        (1920, 0, 1.0, none),
        (1920, 1080, 0.0, none),
        (2560, 1600, 1.0, full),
        (3456, 2234, 2.0, full),
        (1024, 640, 1.0, Some((962.0, 601.0))),
    ];
    for (width, height, scale, expected) in rows {
        assert_eq!(
            monitor_fit_window_size(width, height, scale),
            expected,
            "{width}x{height}@{scale}"
        );
    }
}

#[test]
fn development_builds_refuse_the_production_identity() {
    let dev_id = "com.audiobook-boss.dev.0123456789ab";
    let rows = [
        (true, false, PRODUCTION_IDENTIFIER, true),
        (false, true, PRODUCTION_IDENTIFIER, true),
        (true, true, PRODUCTION_IDENTIFIER, true),
        (true, false, dev_id, false),
        (false, true, dev_id, false),
        (true, true, dev_id, false),
        (false, false, PRODUCTION_IDENTIFIER, false),
    ];
    for (is_dev, debug_build, identifier, refused) in rows {
        assert_eq!(
            refuses_production_identity(is_dev, debug_build, identifier),
            refused,
            "is_dev={is_dev} debug_build={debug_build} identifier={identifier}"
        );
    }
}

#[test]
fn production_identity_matches_tauri_config() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json parses");
    assert_eq!(config["identifier"], PRODUCTION_IDENTIFIER);
}

#[test]
fn window_title_names_non_release_identity_and_revision() {
    let dev_id = "com.audiobook-boss.dev.0123456789ab";
    let rows = [
        (PRODUCTION_IDENTIFIER, "AudioBook Boss", "0123456789ab", None),
        (
            dev_id,
            "AudioBook Boss Dev 012345",
            "0123456789ab-dirty",
            Some("AudioBook Boss Dev 012345 [com.audiobook-boss.dev.0123456789ab · 0123456789ab-dirty]"),
        ),
        (
            dev_id,
            "AudioBook Boss Dev 012345",
            "unknown",
            Some("AudioBook Boss Dev 012345 [com.audiobook-boss.dev.0123456789ab · unknown]"),
        ),
    ];
    for (identifier, product_name, revision, expected) in rows {
        assert_eq!(
            window_title(identifier, product_name, revision).as_deref(),
            expected,
            "{identifier} {revision}"
        );
    }
}
