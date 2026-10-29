#[derive(Debug)]
pub struct PreviewState {
    pub file_count: usize,
    pub per_file_seconds: f64,
    pub current_file_index: usize,
    pub current_file_elapsed_samples: u64,
}

impl PreviewState {
    pub fn new(file_count: usize, per_file_seconds: f64) -> Self {
        Self {
            file_count,
            per_file_seconds,
            current_file_index: 0,
            current_file_elapsed_samples: 0,
        }
    }

    pub fn start_new_file(&mut self, file_index: usize) {
        self.current_file_index = file_index;
        self.current_file_elapsed_samples = 0;
    }

    pub fn all_files_complete(&self) -> bool {
        self.current_file_index + 1 >= self.file_count
    }
}

#[cfg(test)]
#[path = "preview_state_tests.rs"]
mod tests;
