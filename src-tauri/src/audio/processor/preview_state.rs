//! Preview state and chapter marker helpers.

/// Chapter marker for preview output (embedded in M4B)
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
    /// Collected chapter markers for embedding
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
mod tests {
    use super::{sanitize_chapter_title, ChapterMarker, PreviewState};
    use crate::audio::processor::frame_pipeline::PreviewAction;

    #[test]
    fn sanitize_chapter_title_removes_extension() {
        assert_eq!(sanitize_chapter_title("Chapter 01.mp3"), "Chapter 01");
        assert_eq!(sanitize_chapter_title("file.m4a"), "file");
        assert_eq!(sanitize_chapter_title("audio.flac"), "audio");
    }

    #[test]
    fn sanitize_chapter_title_handles_no_extension() {
        assert_eq!(sanitize_chapter_title("Chapter 01"), "Chapter 01");
    }

    #[test]
    fn sanitize_chapter_title_replaces_special_chars() {
        assert_eq!(sanitize_chapter_title("chapter=1.mp3"), "chapter_1");
        assert_eq!(sanitize_chapter_title("chapter[1].mp3"), "chapter_1_");
        assert_eq!(sanitize_chapter_title("chapter#1.mp3"), "chapter_1");
        assert_eq!(sanitize_chapter_title("ch;1.mp3"), "ch_1");
        assert_eq!(sanitize_chapter_title("ch\\1.mp3"), "ch_1");
    }

    #[test]
    fn sanitize_chapter_title_preserves_unicode() {
        assert_eq!(
            sanitize_chapter_title("\u{7B2C}\u{4E00}\u{7AE0}.mp3"),
            "\u{7B2C}\u{4E00}\u{7AE0}"
        );
        assert_eq!(
            sanitize_chapter_title(
                "\u{041A}\u{0430}\u{043F}\u{0438}\u{0442}\u{0443}\u{043B} 1.mp3"
            ),
            "\u{041A}\u{0430}\u{043F}\u{0438}\u{0442}\u{0443}\u{043B} 1"
        );
    }

    #[test]
    fn sanitize_chapter_title_handles_multiple_dots() {
        assert_eq!(sanitize_chapter_title("01.Chapter.mp3"), "01.Chapter");
        assert_eq!(
            sanitize_chapter_title("Track.01.Audio.m4a"),
            "Track.01.Audio"
        );
    }

    #[test]
    fn sanitize_chapter_title_handles_full_path() {
        assert_eq!(
            sanitize_chapter_title("/path/to/Chapter 01.mp3"),
            "Chapter 01"
        );
    }

    #[test]
    fn preview_state_new_initializes_correctly() {
        let state = PreviewState::new(5, 10.0);
        assert_eq!(state.file_count, 5);
        assert!((state.per_file_seconds - 10.0).abs() < 0.001);
        assert_eq!(state.current_file_index, 0);
        assert_eq!(state.current_file_start_pts, 0);
        assert_eq!(state.current_file_elapsed_samples, 0);
        assert!(state.chapter_markers.is_empty());
    }

    #[test]
    fn preview_state_start_new_file_resets_counters() {
        let mut state = PreviewState::new(5, 10.0);
        state.current_file_elapsed_samples = 48_000;
        state.start_new_file(2, "Track03.mp3", 144_000);

        assert_eq!(state.current_file_index, 2);
        assert_eq!(state.current_file_name, "Track03.mp3");
        assert_eq!(state.current_file_start_pts, 144_000);
        assert_eq!(state.current_file_elapsed_samples, 0);
    }

    #[test]
    fn preview_state_all_files_complete() {
        let mut state = PreviewState::new(3, 10.0);
        assert!(!state.all_files_complete());

        state.current_file_index = 1;
        assert!(!state.all_files_complete());

        state.current_file_index = 2;
        assert!(state.all_files_complete());
    }

    #[test]
    fn preview_state_record_chapter() {
        let mut state = PreviewState::new(3, 10.0);
        state.current_file_name = "Chapter 01.mp3".to_string();
        state.current_file_start_pts = 0;

        state.record_chapter(48_000, 48_000);

        assert_eq!(state.chapter_markers.len(), 1);
        assert_eq!(state.chapter_markers[0].start_ms, 0);
        assert_eq!(state.chapter_markers[0].end_ms, 1000);
        assert_eq!(state.chapter_markers[0].title, "Chapter 01");
    }

    #[test]
    fn preview_action_enum_values() {
        assert_eq!(PreviewAction::Continue, PreviewAction::Continue);
        assert_ne!(PreviewAction::Continue, PreviewAction::NextFile);
        assert_ne!(PreviewAction::NextFile, PreviewAction::StopAll);
    }

    #[test]
    fn chapter_marker_struct() {
        let marker = ChapterMarker {
            start_ms: 0,
            end_ms: 5000,
            title: "Intro".to_string(),
        };
        assert_eq!(marker.start_ms, 0);
        assert_eq!(marker.end_ms, 5000);
        assert_eq!(marker.title, "Intro");
    }
}
