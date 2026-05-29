//! Preview state helpers.

/// Extended state for adaptive multi-file preview
#[derive(Debug)]
pub struct PreviewState {
    /// Total number of files being processed
    pub file_count: usize,
    /// Calculated per-file duration (seconds)
    pub per_file_seconds: f64,
    /// Current file index (0-based)
    pub current_file_index: usize,
    /// Samples processed for current file excerpt
    pub current_file_elapsed_samples: u64,
}

impl PreviewState {
    /// Creates a new PreviewState for adaptive preview
    pub fn new(file_count: usize, per_file_seconds: f64) -> Self {
        Self {
            file_count,
            per_file_seconds,
            current_file_index: 0,
            current_file_elapsed_samples: 0,
        }
    }

    /// Resets per-file counters when switching to a new file
    pub fn start_new_file(&mut self, file_index: usize) {
        self.current_file_index = file_index;
        self.current_file_elapsed_samples = 0;
    }

    /// Returns true if all files have been processed
    pub fn all_files_complete(&self) -> bool {
        self.current_file_index + 1 >= self.file_count
    }
}

#[cfg(test)]
#[path = "preview_state_tests.rs"]
mod tests;
