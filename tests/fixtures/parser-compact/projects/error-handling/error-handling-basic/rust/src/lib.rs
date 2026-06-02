pub fn parse_score(text: &str) -> Result<i32, String> {
    let value = text
        .parse::<i32>()
        .map_err(|_| "invalid".to_string())?;
    if value < 0 {
        return Err("negative".to_string());
    }
    Ok(value)
}
