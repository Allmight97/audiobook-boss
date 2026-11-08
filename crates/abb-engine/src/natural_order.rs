use std::cmp::Ordering;

pub(crate) fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left_folded: Vec<char> = left.to_lowercase().chars().collect();
    let right_folded: Vec<char> = right.to_lowercase().chars().collect();
    compare_folded(&left_folded, &right_folded).then_with(|| left.cmp(right))
}

fn compare_folded(left: &[char], right: &[char]) -> Ordering {
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if left[i].is_ascii_digit() && right[j].is_ascii_digit() {
            let (left_run, left_end) = digit_run(left, i);
            let (right_run, right_end) = digit_run(right, j);
            let ordering = left_run
                .len()
                .cmp(&right_run.len())
                .then_with(|| left_run.cmp(&right_run));
            if ordering != Ordering::Equal {
                return ordering;
            }
            i = left_end;
            j = right_end;
        } else {
            let ordering = left[i].cmp(&right[j]);
            if ordering != Ordering::Equal {
                return ordering;
            }
            i += 1;
            j += 1;
        }
    }
    (left.len() - i).cmp(&(right.len() - j))
}

fn digit_run(chars: &[char], start: usize) -> (String, usize) {
    let end = chars[start..]
        .iter()
        .position(|c| !c.is_ascii_digit())
        .map_or(chars.len(), |offset| start + offset);
    let run: String = chars[start..end].iter().collect();
    (run.trim_start_matches('0').to_string(), end)
}

#[cfg(test)]
#[path = "natural_order_tests.rs"]
mod tests;
