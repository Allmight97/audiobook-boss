use super::*;

const LIMIT: i64 = 9_007_199_254_740_991;

#[test]
fn unsigned_accepts_the_largest_exact_js_integer_and_rejects_the_next() {
    assert_eq!(
        JsSafeU64::try_from(LIMIT as u64).map(JsSafeU64::get),
        Ok(LIMIT as u64)
    );
    assert_eq!(
        JsSafeU64::try_from(LIMIT as u64 + 1),
        Err(OutsideJsSafeRange(i128::from(LIMIT) + 1))
    );
}

#[test]
fn signed_accepts_both_exact_js_bounds_and_rejects_beyond_them() {
    assert_eq!(JsSafeI64::try_from(LIMIT).map(JsSafeI64::get), Ok(LIMIT));
    assert_eq!(JsSafeI64::try_from(-LIMIT).map(JsSafeI64::get), Ok(-LIMIT));
    assert_eq!(
        JsSafeI64::try_from(LIMIT + 1),
        Err(OutsideJsSafeRange(i128::from(LIMIT) + 1))
    );
    assert_eq!(
        JsSafeI64::try_from(-LIMIT - 1),
        Err(OutsideJsSafeRange(-i128::from(LIMIT) - 1))
    );
}

#[test]
fn wire_json_is_a_plain_number_and_out_of_range_input_is_refused() {
    let unsigned = JsSafeU64::try_from(LIMIT as u64).expect("JS-safe test value");
    assert_eq!(
        serde_json::to_string(&unsigned).expect("JSON round trip"),
        "9007199254740991"
    );
    assert_eq!(
        serde_json::from_str::<JsSafeU64>("9007199254740991").expect("JSON round trip"),
        unsigned
    );
    assert!(serde_json::from_str::<JsSafeU64>("9007199254740992").is_err());

    let signed = JsSafeI64::try_from(-LIMIT).expect("JS-safe test value");
    assert_eq!(
        serde_json::to_string(&signed).expect("JSON round trip"),
        "-9007199254740991"
    );
    assert!(serde_json::from_str::<JsSafeI64>("-9007199254740992").is_err());
    assert!(serde_json::from_str::<JsSafeI64>("9007199254740992").is_err());
}
