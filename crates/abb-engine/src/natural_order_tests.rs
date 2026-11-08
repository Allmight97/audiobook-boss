use super::*;

#[test]
fn digit_runs_compare_by_value_and_text_ignores_case() {
    assert_eq!(natural_cmp("Part 2", "Part 10"), Ordering::Less);
    assert_eq!(natural_cmp("part 1", "PART 1x"), Ordering::Less);
    assert_eq!(
        natural_cmp("\u{dc}ber 10", "\u{fc}ber 2"),
        Ordering::Greater
    );
}

#[test]
fn names_that_differ_only_in_leading_zeros_never_compare_equal() {
    assert_ne!(natural_cmp("01.mp3", "1.mp3"), Ordering::Equal);
    assert_eq!(
        natural_cmp("01.mp3", "1.mp3"),
        natural_cmp("1.mp3", "01.mp3").reverse()
    );
}

#[test]
fn names_that_differ_only_in_case_never_compare_equal() {
    assert_ne!(natural_cmp("Book.mp3", "book.mp3"), Ordering::Equal);
}

#[test]
fn a_name_sorts_before_the_same_name_followed_by_a_digit() {
    assert_eq!(natural_cmp("Book.mp3", "Book1.mp3"), Ordering::Less);
}
