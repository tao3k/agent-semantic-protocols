use serde::Serialize;

use crate::canonical_json::write_canonical_json;

pub fn canonical_json_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    let mut canonical = String::new();
    write_canonical_json(&value, &mut canonical)?;
    Ok(format!(
        "blake3:{}",
        blake3::hash(canonical.as_bytes()).to_hex()
    ))
}
