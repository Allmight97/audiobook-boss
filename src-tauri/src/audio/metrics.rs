//! Audio processing metrics tracking
//! 
//! This module provides metrics tracking for audio processing operations,
//! including throughput calculation and performance monitoring.

use std::time::{Duration, Instant};

/// Metrics tracker for audio processing operations
#[derive(Debug)]
pub struct ProcessingMetrics {
    /// Start time of processing
    start_time: Instant,
    /// Number of files processed
    files_processed: usize,
    /// Total duration of audio processed
    total_duration: Duration,
    /// Total bytes processed
    bytes_processed: usize,
}

impl ProcessingMetrics {
    /// Creates a new ProcessingMetrics instance
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            files_processed: 0,
            total_duration: Duration::ZERO,
            bytes_processed: 0,
        }
    }

    /// Updates metrics when a file has been processed
    pub fn update_file_processed(&mut self, duration: Duration, bytes: usize) {
        self.files_processed += 1;
        self.total_duration += duration;
        self.bytes_processed += bytes;
    }

    /// Returns elapsed time since processing started
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Calculates throughput in megabytes per second
    pub fn throughput_mbps(&self) -> f64 {
        let elapsed_secs = self.elapsed().as_secs_f64();
        if elapsed_secs > 0.0 {
            (self.bytes_processed as f64 / 1_048_576.0) / elapsed_secs
        } else {
            0.0
        }
    }

    /// Formats a summary of processing metrics
    pub fn format_summary(&self) -> String {
        let elapsed = self.elapsed();
        let throughput = self.throughput_mbps();
        let audio_hours = self.total_duration.as_secs_f64() / 3600.0;
        let mb_processed = self.bytes_processed as f64 / 1_048_576.0;
        
        format!(
            "Processing Complete:\n\
             - Files processed: {}\n\
             - Audio duration: {:.2} hours\n\
             - Data processed: {:.2} MB\n\
             - Time elapsed: {}m {}s\n\
             - Throughput: {:.2} MB/s",
            self.files_processed,
            audio_hours,
            mb_processed,
            elapsed.as_secs() / 60,
            elapsed.as_secs() % 60,
            throughput
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        
        // Add 10 MB of data
        metrics.update_file_processed(Duration::from_secs(60), 10_485_760);
        
        // Sleep to ensure some time has elapsed
        thread::sleep(Duration::from_millis(100));
        
        let throughput = metrics.throughput_mbps();
        assert!(throughput > 0.0);
    }

    #[test]
    fn test_format_summary() {
        let mut metrics = ProcessingMetrics::new();
        
        // Add some test data
        metrics.update_file_processed(Duration::from_secs(3600), 5_242_880); // 1 hour, 5 MB
        metrics.update_file_processed(Duration::from_secs(1800), 3_145_728); // 30 min, 3 MB
        
        let summary = metrics.format_summary();
        
        assert!(summary.contains("Files processed: 2"));
        assert!(summary.contains("Audio duration: 1.50 hours"));
        assert!(summary.contains("Data processed: 8.00 MB"));
        assert!(summary.contains("Throughput:"));
    }
}