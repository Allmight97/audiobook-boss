use super::PreviewConfig;

#[test]
fn a_title_past_eighteen_files_previews_longer_than_ninety_seconds() {
    assert_eq!(PreviewConfig.title_seconds(1).round(), 90.0);
    assert_eq!(PreviewConfig.title_seconds(7).round(), 90.0);
    assert_eq!(PreviewConfig.title_seconds(18).round(), 90.0);
    assert_eq!(PreviewConfig.title_seconds(30).round(), 150.0);
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
