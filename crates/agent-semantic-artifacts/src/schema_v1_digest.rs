use serde_json::Value;

use crate::blake3_content_digest::Blake3ContentDigest;

pub(crate) fn canonicalize_schema_v1_blake3_value(
    value: Value,
) -> Result<Blake3ContentDigest, String> {
    let source = match value {
        Value::String(source) => source,
        Value::Object(identity) => {
            let algorithm = identity
                .get("algorithm")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_v1_error("missing algorithm"))?;
            if algorithm != "blake3-256" {
                return Err(schema_v1_error("algorithm is not blake3-256"));
            }
            identity
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(|| schema_v1_error("missing value"))?
                .to_owned()
        }
        _ => return Err(schema_v1_error("identity is neither string nor object")),
    };

    let canonical = if source.starts_with("blake3-256:") {
        source
    } else if source.len() == 64
        && source
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        format!("blake3-256:{source}")
    } else {
        return Err(schema_v1_error("digest is not canonical BLAKE3"));
    };
    Blake3ContentDigest::parse(&canonical).map_err(|error| schema_v1_error(&error))
}

fn schema_v1_error(detail: &str) -> String {
    format!("reasonKind=artifact-identity-incomplete schemaVersion=1 {detail}")
}

#[cfg(test)]
#[path = "../tests/unit/schema_v1_digest.rs"]
mod tests;
