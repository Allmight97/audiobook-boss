//! Preview state and chapter marker helpers.

/// Chapter marker collected for preview output.
// TODO(audio-preview): wire collected markers into preview chapter emission or
// remove marker collection if adaptive previews no longer need it.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChapterMarker {
    /// Chapter start time in milliseconds
    pub start_ms: i64,
    /// Chapter end time in milliseconds
    pub end_ms: i64,
    /// Chapter title (sanitized filename)
    pub title: String,
}

/// Extended state for adaptive multi-file preview
#[derive(Debug)]
pub struct PreviewState {
    /// Total number of files being processed
    pub file_count: usize,
    /// Calculated per-file duration (seconds)
    pub per_file_seconds: f64,
    /// Current file index (0-based)
    pub current_file_index: usize,
    /// PTS at start of current file excerpt
    pub current_file_start_pts: i64,
    /// Samples processed for current file excerpt
    pub current_file_elapsed_samples: u64,
    /// Collected chapter markers for preview chapter emission
    pub chapter_markers: Vec<ChapterMarker>,
    /// Current file name for chapter title
    pub current_file_name: String,
}

impl PreviewState {
    /// Creates a new PreviewState for adaptive preview
    pub fn new(file_count: usize, per_file_seconds: f64) -> Self {
        Self {
            file_count,
            per_file_seconds,
            current_file_index: 0,
            current_file_start_pts: 0,
            current_file_elapsed_samples: 0,
            chapter_markers: Vec::with_capacity(file_count),
            current_file_name: String::new(),
        }
    }

    /// Resets per-file counters when switching to a new file
    pub fn start_new_file(&mut self, file_index: usize, file_name: &str, current_pts: i64) {
        self.current_file_index = file_index;
        self.current_file_name = file_name.to_string();
        self.current_file_start_pts = current_pts;
        self.current_file_elapsed_samples = 0;
    }

    /// Records a chapter marker for the current file excerpt
    pub fn record_chapter(&mut self, end_pts: i64, sample_rate: u32) {
        let start_ms = (self.current_file_start_pts * 1000) / sample_rate as i64;
        let end_ms = (end_pts * 1000) / sample_rate as i64;
        let title = sanitize_chapter_title(&self.current_file_name);
        self.chapter_markers.push(ChapterMarker {
            start_ms,
            end_ms,
            title,
        });
    }

    /// Returns true if all files have been processed
    pub fn all_files_complete(&self) -> bool {
        self.current_file_index + 1 >= self.file_count
    }
}

/// Sanitize filename for FFMETADATA chapter title
pub fn sanitize_chapter_title(filename: &str) -> String {
    // Remove extension
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    // Replace FFMETADATA special characters
    stem.chars()
        .map(|c| match c {
            '=' | '[' | ']' | '#' | ';' | '\\' | '\n' | '\r' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
#[path = "preview_state_tests.rs"]
mod tests;
