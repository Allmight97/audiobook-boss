pub const PREVIEW_SECONDS: f64 = 90.0;
const PREVIEW_MIN_SEGMENT_SECONDS: f64 = 5.0;

pub fn per_file_seconds(file_count: usize) -> f64 {
    (PREVIEW_SECONDS / file_count as f64).max(PREVIEW_MIN_SEGMENT_SECONDS)
}

pub fn title_seconds(file_durations: &[f64]) -> f64 {
    let share = per_file_seconds(file_durations.len());
    file_durations
        .iter()
        .map(|duration| duration.min(share))
        .sum()
}

pub fn per_file_samples(file_count: usize, rate: u32) -> u64 {
    (per_file_seconds(file_count) * f64::from(rate)).round() as u64
}

#[cfg(test)]
#[path = "preview_config_tests.rs"]
mod tests;
