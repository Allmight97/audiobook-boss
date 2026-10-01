//! Preview configuration helpers.

/// Minimum segment duration per file (in seconds) for adaptive preview
pub const PREVIEW_MIN_SEGMENT_SECONDS: f64 = 5.0;

/// Preview configuration for early-stop preview encodes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreviewConfig {
    /// Total preview duration requested by user (15/30/45/60s)
    pub total_seconds: f64,
    /// Minimum segment per file (default 5.0s)
    pub min_segment_seconds: f64,
}

impl PreviewConfig {
    /// Creates a new preview configuration with the given total duration
    pub fn new(total_seconds: f64) -> Self {
        Self {
            total_seconds,
            min_segment_seconds: PREVIEW_MIN_SEGMENT_SECONDS,
        }
    }

    /// Calculate per-file excerpt duration based on file count
    ///
    /// The duration is divided equally across files, with a floor at
    /// `min_segment_seconds` to avoid fragments that are too short.
    pub fn per_file_seconds(&self, file_count: usize) -> f64 {
        if file_count == 0 {
            return self.total_seconds;
        }
        let calculated = self.total_seconds / file_count as f64;
        calculated.max(self.min_segment_seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::PreviewConfig;

    #[test]
    fn per_file_seconds_divides_requested_duration() {
        let config = PreviewConfig::new(30.0);
        assert!((config.per_file_seconds(3) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn per_file_seconds_applies_minimum_segment_floor() {
        let config = PreviewConfig::new(30.0);
        assert!((config.per_file_seconds(7) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn per_file_seconds_preserves_single_file_duration() {
        let config = PreviewConfig::new(30.0);
        assert!((config.per_file_seconds(1) - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn per_file_seconds_uses_total_for_zero_files() {
        let config = PreviewConfig::new(30.0);
        assert!((config.per_file_seconds(0) - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn per_file_seconds_keeps_exact_floor_boundary() {
        let config = PreviewConfig::new(30.0);
        assert!((config.per_file_seconds(6) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn per_file_seconds_scales_with_different_preview_durations() {
        assert!((PreviewConfig::new(15.0).per_file_seconds(3) - 5.0).abs() < f64::EPSILON);
        assert!((PreviewConfig::new(45.0).per_file_seconds(3) - 15.0).abs() < f64::EPSILON);
        assert!((PreviewConfig::new(60.0).per_file_seconds(4) - 15.0).abs() < f64::EPSILON);
    }
}
