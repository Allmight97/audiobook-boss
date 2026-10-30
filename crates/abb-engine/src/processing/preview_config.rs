pub const PREVIEW_MIN_SEGMENT_SECONDS: f64 = 5.0;

pub(crate) fn is_valid_preview_length(seconds: f64) -> bool {
    seconds.is_finite() && seconds > 0.0
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreviewConfig {
    pub total_seconds: f64,
    pub min_segment_seconds: f64,
}

impl PreviewConfig {
    pub fn new(total_seconds: f64) -> Self {
        Self {
            total_seconds,
            min_segment_seconds: PREVIEW_MIN_SEGMENT_SECONDS,
        }
    }

    pub fn per_file_seconds(&self, file_count: usize) -> f64 {
        if file_count == 0 {
            return self.total_seconds;
        }
        let calculated = self.total_seconds / file_count as f64;
        calculated.max(self.min_segment_seconds)
    }
}

#[cfg(test)]
#[path = "preview_config_tests.rs"]
mod tests;
