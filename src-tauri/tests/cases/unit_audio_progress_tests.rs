use audiobook_boss_lib::processing::progress::{
    calculate_stage_progress, converting_percentage_from_seconds, format_eta, ProgressReporter,
};
use audiobook_boss_lib::processing::ProcessingStage;

#[test]
fn test_calculate_stage_progress() {
    // Test progress calculation within a stage
    assert_eq!(calculate_stage_progress(0.0, 100.0, 10.0, 80.0), 10.0);
    assert_eq!(calculate_stage_progress(50.0, 100.0, 10.0, 80.0), 45.0);
    assert_eq!(calculate_stage_progress(100.0, 100.0, 10.0, 80.0), 80.0);

    // Test edge cases
    assert_eq!(calculate_stage_progress(50.0, 0.0, 10.0, 80.0), 10.0);
}

#[test]
fn test_format_eta() {
    assert_eq!(format_eta(30.0), "30s");
    assert_eq!(format_eta(90.0), "1m 30s");
    assert_eq!(format_eta(150.5), "2m 30s");
    assert_eq!(format_eta(0.0), "0s");
    assert_eq!(format_eta(59.9), "60s");
    assert_eq!(format_eta(60.0), "1m 0s");
    assert_eq!(format_eta(125.0), "2m 5s");
}

#[test]
fn test_converting_percentage_from_seconds() {
    // Test basic calculation
    assert!(converting_percentage_from_seconds(0.0, 100.0) >= 10.0);
    assert!(converting_percentage_from_seconds(50.0, 100.0) > 10.0);
    assert!(converting_percentage_from_seconds(100.0, 100.0) <= 80.0);

    // Test edge cases
    assert_eq!(
        converting_percentage_from_seconds(0.0, 0.0),
        10.0 // PROGRESS_CONVERTING_START
    );
    assert!(converting_percentage_from_seconds(100.0, 0.0) >= 10.0);
}

#[test]
fn test_progress_reporter_new() {
    let reporter = ProgressReporter::new(5);

    // Use public API to access state
    let progress = reporter.get_progress();
    assert_eq!(progress.total_files, 5);
    assert_eq!(progress.files_completed, 0);
    assert!(matches!(progress.stage, ProcessingStage::Analyzing));
}

#[test]
fn test_calculate_progress() {
    let mut reporter = ProgressReporter::new(4);

    // Initial progress
    assert_eq!(reporter.calculate_progress(), 0.0);

    // Complete analyzing stage
    reporter.complete_file();
    reporter.set_stage(ProcessingStage::Converting);
    assert!(reporter.calculate_progress() > 10.0);

    // Complete all files
    reporter.complete();
    assert_eq!(reporter.calculate_progress(), 100.0);
}

#[test]
fn test_estimate_time_remaining() {
    let reporter = ProgressReporter::new(2);
    // At 0% progress, should return None
    assert!(reporter.estimate_time_remaining().is_none());
}

#[test]
fn test_get_progress() {
    let mut reporter = ProgressReporter::new(3);
    reporter.set_stage(ProcessingStage::Converting);
    reporter.set_current_file("test.m4b");

    let progress = reporter.get_progress();
    assert!(matches!(progress.stage, ProcessingStage::Converting));
    assert_eq!(progress.current_file, Some("test.m4b".to_string()));
    assert_eq!(progress.total_files, 3);
    assert_eq!(progress.files_completed, 0);
}

#[test]
fn test_complete_file() {
    let mut reporter = ProgressReporter::new(2);
    reporter.set_current_file("test.m4b");

    reporter.complete_file();

    let progress = reporter.get_progress();
    assert_eq!(progress.files_completed, 1);
    assert_eq!(progress.current_file, None);
}

#[test]
fn test_complete() {
    let mut reporter = ProgressReporter::new(5);

    reporter.complete();

    let progress = reporter.get_progress();
    assert!(matches!(progress.stage, ProcessingStage::Completed));
    assert_eq!(progress.files_completed, 5);
    assert_eq!(progress.current_file, None);
    assert_eq!(progress.progress, 100.0);
}

#[test]
fn test_fail() {
    let mut reporter = ProgressReporter::new(5);
    reporter.set_current_file("test.m4b");

    reporter.fail("Test error");

    let progress = reporter.get_progress();
    assert!(matches!(progress.stage, ProcessingStage::Failed(_)));
    assert_eq!(progress.current_file, None);
}
