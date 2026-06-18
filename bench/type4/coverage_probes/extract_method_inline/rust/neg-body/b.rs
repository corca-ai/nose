fn square_plus_one(value: i64) -> i64 {
    value * value + 2
}

pub fn axis_case(x: i64, y: i64) -> i64 {
    let total = x + y;
    square_plus_one(total)
}
