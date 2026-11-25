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
    pub(super) fn offer(&mut self, is_front: bool, picture: T) -> Option<T> {
        if is_front {
            return Some(picture);
        }
        self.first.get_or_insert(picture);
        None
    }

    pub(super) fn fallback(self) -> Option<T> {
        self.first
    }
}

#[cfg(test)]
#[path = "cover_choice_tests.rs"]
mod tests;
