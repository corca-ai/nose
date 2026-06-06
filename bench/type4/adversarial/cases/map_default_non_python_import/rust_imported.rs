use tables::LOOKUP;

pub fn lookup(key: &str, other: &str) -> i32 {
    *std::collections::HashMap::from(LOOKUP).get(key).unwrap_or(&0)
}
