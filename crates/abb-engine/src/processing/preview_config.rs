/// The preview length the Preview Audio action renders for each title.
pub const PREVIEW_SECONDS: f64 = 90.0;
const PREVIEW_MIN_SEGMENT_SECONDS: f64 = 5.0;

/// A preview: the start of each file, `PREVIEW_SECONDS` split across the
/// files, never under `PREVIEW_MIN_SEGMENT_SECONDS` each.
#[derive(Debug, Clone, Copy)]
pub struct PreviewConfig;

impl PreviewConfig {
    pub fn per_file_seconds(&self, file_count: usize) -> f64 {
        (PREVIEW_SECONDS / file_count as f64).max(PREVIEW_MIN_SEGMENT_SECONDS)
    }

    /// How long a title's preview runs, its files' excerpts end to end.
    pub(crate) fn title_seconds(&self, file_count: usize) -> f64 {
        self.per_file_seconds(file_count) * file_count as f64
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
