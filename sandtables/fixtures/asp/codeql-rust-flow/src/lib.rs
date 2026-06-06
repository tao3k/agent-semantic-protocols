pub fn source_value(input: i32) -> i32 {
    input + 1
}

pub fn sink_value(value: i32) -> i32 {
    source_value(value) * 2
}
