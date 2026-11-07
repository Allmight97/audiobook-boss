//! Which chapters ABB sends to the output when titles' sources are joined.

pub(crate) fn synthesizes_chapter_per_source(source_count: usize, chapter_total: usize) -> bool {
    source_count > 1 && chapter_total == 0
}

pub(crate) fn output_chapter_count(source_chapter_counts: &[usize]) -> usize {
    let total = source_chapter_counts.iter().sum();
    if synthesizes_chapter_per_source(source_chapter_counts.len(), total) {
        source_chapter_counts.len()
    } else {
        total
    }
}

#[cfg(test)]
#[path = "chapter_count_tests.rs"]
mod tests;
