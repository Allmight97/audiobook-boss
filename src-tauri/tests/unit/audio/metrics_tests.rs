use audiobook_boss_lib::audio::metrics::ProcessingMetrics;
use std::time::Duration;
use std::thread;

#[test]
fn test_new_metrics() {
    let metrics = ProcessingMetrics::new();
    assert_eq!(metrics.files_processed, 0);
    assert_eq!(metrics.bytes_processed, 0);
    assert_eq!(metrics.total_duration, Duration::ZERO);
}

#[test]
fn test_update_file_processed() {
    let mut metrics = ProcessingMetrics::new();
    let duration = Duration::from_secs(60);
    let bytes = 1_048_576; // 1 MB
    metrics.update_file_processed(duration, bytes);
    assert_eq!(metrics.files_processed, 1);
    assert_eq!(metrics.bytes_processed, bytes);
    assert_eq!(metrics.total_duration, duration);
}

#[test]
fn test_elapsed_time() {
    let metrics = ProcessingMetrics::new();
    thread::sleep(Duration::from_millis(10));
    let elapsed = metrics.elapsed();
    assert!(elapsed >= Duration::from_millis(10));
}

#[test]
fn test_throughput_calculation() {
    let mut metrics = ProcessingMetrics::new();
    metrics.update_file_processed(Duration::from_secs(60), 10_485_760);
    thread::sleep(Duration::from_millis(100));
    let throughput = metrics.throughput_mbps();
    assert!(throughput > 0.0);
}

#[test]
fn test_format_summary() {
    let mut metrics = ProcessingMetrics::new();
    metrics.update_file_processed(Duration::from_secs(3600), 5_242_880); // 1 hour, 5 MB
    metrics.update_file_processed(Duration::from_secs(1800), 3_145_728); // 30 min, 3 MB
    let summary = metrics.format_summary();
    assert!(summary.contains("Files processed: 2"));
    assert!(summary.contains("Audio duration: 1.50 hours"));
    assert!(summary.contains("Data processed: 8.00 MB"));
    assert!(summary.contains("Throughput:"));
}


