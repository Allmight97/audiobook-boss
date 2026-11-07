//! Which of a file's pictures is its cover: the front cover (ID3 APIC or FLAC
//! picture type 3; a Matroska attachment named like a cover), else the first
//! picture. The thumbnail readers and the FFmpeg export path offer their
//! pictures in file order, so the app shows the cover an export keeps.

#[derive(Debug)]
pub(super) struct CoverChoice<T> {
    first: Option<T>,
}

impl<T> Default for CoverChoice<T> {
    fn default() -> Self {
        Self { first: None }
    }
}

impl<T> CoverChoice<T> {
    /// Offers the next picture. The front cover ends the choice and comes back
    /// at once; any other is remembered if it is the first.
    pub(super) fn offer(&mut self, is_front: bool, picture: T) -> Option<T> {
        if is_front {
            return Some(picture);
        }
        self.first.get_or_insert(picture);
        None
    }

    /// The first picture offered, once no front cover turned up.
    pub(super) fn fallback(self) -> Option<T> {
        self.first
    }
}

#[cfg(test)]
#[path = "cover_choice_tests.rs"]
mod tests;
