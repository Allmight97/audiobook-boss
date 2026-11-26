use std::fmt;

use serde::{Deserialize, Serialize};
use specta::{datatype::DataType, Type, Types};

const MAX_JS_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutsideJsSafeRange(pub i128);

impl fmt::Display for OutsideJsSafeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is outside the integer range a JavaScript number holds exactly",
            self.0
        )
    }
}

impl std::error::Error for OutsideJsSafeRange {}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(into = "u64", try_from = "u64")]
pub struct JsSafeU64(u64);

impl JsSafeU64 {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for JsSafeU64 {
    type Error = OutsideJsSafeRange;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value <= MAX_JS_SAFE_INTEGER as u64 {
            Ok(Self(value))
        } else {
            Err(OutsideJsSafeRange(value.into()))
        }
    }
}

impl From<JsSafeU64> for u64 {
    fn from(value: JsSafeU64) -> Self {
        value.0
    }
}

impl fmt::Display for JsSafeU64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Type for JsSafeU64 {
    fn definition(types: &mut Types) -> DataType {
        specta_typescript::Number::<u64>::definition(types)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "i64", try_from = "i64")]
pub struct JsSafeI64(i64);

impl JsSafeI64 {
    pub fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for JsSafeI64 {
    type Error = OutsideJsSafeRange;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if (-MAX_JS_SAFE_INTEGER..=MAX_JS_SAFE_INTEGER).contains(&value) {
            Ok(Self(value))
        } else {
            Err(OutsideJsSafeRange(value.into()))
        }
    }
}

impl From<JsSafeI64> for i64 {
    fn from(value: JsSafeI64) -> Self {
        value.0
    }
}

impl fmt::Display for JsSafeI64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Type for JsSafeI64 {
    fn definition(types: &mut Types) -> DataType {
        specta_typescript::Number::<i64>::definition(types)
    }
}

#[cfg(test)]
#[path = "js_safe_tests.rs"]
mod tests;
