/// The current file's preview excerpt, cut at exactly `per_file_samples`.
#[derive(Debug)]
pub struct PreviewState {
    per_file_samples: u64,
    current_file_elapsed_samples: u64,
}

impl PreviewState {
    pub fn new(per_file_samples: u64) -> Self {
        Self {
            per_file_samples,
            current_file_elapsed_samples: 0,
        }
    }

    pub fn start_new_file(&mut self) {
        self.current_file_elapsed_samples = 0;
    }

    /// Samples the current excerpt can still take from a frame of `available`.
    pub fn take(&mut self, available: usize) -> usize {
        let left = self
            .per_file_samples
            .saturating_sub(self.current_file_elapsed_samples);
        let taken = left.min(available as u64);
        self.current_file_elapsed_samples += taken;
        taken as usize
    }

    pub fn excerpt_complete(&self) -> bool {
        self.current_file_elapsed_samples >= self.per_file_samples
    }
}

#[cfg(test)]
#[path = "preview_state_tests.rs"]
mod tests;
