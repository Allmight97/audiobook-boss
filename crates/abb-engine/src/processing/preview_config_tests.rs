use super::PreviewConfig;

#[test]
fn a_title_past_eighteen_files_previews_longer_than_ninety_seconds() {
    for (files, seconds) in [(1, 90.0), (7, 90.0), (18, 90.0), (30, 150.0)] {
        let durations = vec![600.0; files];
        assert_eq!(PreviewConfig.title_seconds(&durations).round(), seconds);
    }
}

#[test]
fn a_file_shorter_than_its_share_adds_only_its_own_length() {
    assert_eq!(PreviewConfig.title_seconds(&[10.0]), 10.0);
    assert_eq!(PreviewConfig.title_seconds(&[10.0, 600.0]), 55.0);
}

#[test]
fn ninety_seconds_split_across_the_files_with_a_five_second_floor() {
    for (files, seconds, samples_at_44k1, samples_at_22k05) in [
        (1, 90.0, 3_969_000, 1_984_500),
        (5, 18.0, 793_800, 396_900),
        (18, 5.0, 220_500, 110_250),
        (30, 5.0, 220_500, 110_250),
    ] {
        assert!(
            (PreviewConfig.per_file_seconds(files) - seconds).abs() < f64::EPSILON,
            "{files} files"
        );
        assert_eq!(
            PreviewConfig.per_file_samples(files, 44_100),
            samples_at_44k1
        );
        assert_eq!(
            PreviewConfig.per_file_samples(files, 22_050),
            samples_at_22k05
        );
    }
}
