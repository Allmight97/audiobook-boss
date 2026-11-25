pub mod fs_listing;
pub mod log_capture;
pub mod media;
pub mod poll;
pub mod wav;

#[expect(
    clippy::disallowed_methods,
    reason = "tests move their own temp files to stage a scenario"
)]
pub fn move_test_file(from: &std::path::Path, to: &std::path::Path) {
    std::fs::rename(from, to).expect("move a test file");
}
