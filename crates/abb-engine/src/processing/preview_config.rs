/// The preview length the Preview Audio action renders for each title.
pub const PREVIEW_SECONDS: f64 = 90.0;
const PREVIEW_MIN_SEGMENT_SECONDS: f64 = 5.0;

#[derive(Debug, Clone)]
pub struct PreviewConfig {
    pub total_seconds: f64,
}

impl PreviewConfig {
    pub fn new(total_seconds: f64) -> Self {
        Self { total_seconds }
    }

    pub fn per_file_seconds(&self, file_count: usize) -> f64 {
        (self.total_seconds / file_count as f64).max(PREVIEW_MIN_SEGMENT_SECONDS)
    }

    /// Each file's excerpt length at the output rate; the frame pipeline cuts
    /// every excerpt at exactly this sample.
    pub fn per_file_samples(&self, file_count: usize, rate: u32) -> u64 {
        (self.per_file_seconds(file_count) * f64::from(rate)).round() as u64
    }
}

#[cfg(test)]
#[path = "preview_config_tests.rs"]
mod tests;
