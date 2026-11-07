use super::CoverChoice;

fn choose(pictures: &[(bool, &'static str)]) -> Option<&'static str> {
    let mut choice = CoverChoice::default();
    for &(is_front, picture) in pictures {
        if let Some(front) = choice.offer(is_front, picture) {
            return Some(front);
        }
    }
    choice.fallback()
}

#[test]
fn the_front_cover_wins_wherever_it_sits() {
    assert_eq!(
        choose(&[(false, "other"), (true, "front"), (false, "back")]),
        Some("front")
    );
    assert_eq!(choose(&[(true, "front"), (false, "other")]), Some("front"));
}

#[test]
fn without_a_front_cover_the_first_picture_wins() {
    assert_eq!(choose(&[(false, "one"), (false, "two")]), Some("one"));
}

#[test]
fn no_pictures_give_no_cover() {
    assert_eq!(choose(&[]), None);
}
