/// The preview length the Preview Audio action renders for each title.
pub const PREVIEW_SECONDS: f64 = 90.0;
const PREVIEW_MIN_SEGMENT_SECONDS: f64 = 5.0;

/// A preview is the start of each file: `PREVIEW_SECONDS` split across the
/// files, never under `PREVIEW_MIN_SEGMENT_SECONDS` each.
pub fn per_file_seconds(file_count: usize) -> f64 {
    (PREVIEW_SECONDS / file_count as f64).max(PREVIEW_MIN_SEGMENT_SECONDS)
}

/// How long a title's preview runs, its files' excerpts end to end. A file
/// shorter than its share adds only its own length.
pub fn title_seconds(file_durations: &[f64]) -> f64 {
    let share = per_file_seconds(file_durations.len());
    file_durations
        .iter()
        .map(|duration| duration.min(share))
        .sum()
}

/// Each file's excerpt length at the output rate; the frame pipeline cuts
/// every excerpt at exactly this sample.
pub fn per_file_samples(file_count: usize, rate: u32) -> u64 {
    (per_file_seconds(file_count) * f64::from(rate)).round() as u64
}

#[cfg(test)]
#[path = "preview_config_tests.rs"]
mod tests;
