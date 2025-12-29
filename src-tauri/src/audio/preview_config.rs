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
