pub(super) fn payload_command_strings(payload: &serde_json::Value) -> Vec<String> {
    let mut commands = Vec::new();
    collect_payload_command_strings(payload, &mut commands);
    commands.sort();
    commands.dedup();
    commands
}

pub(super) fn payload_evidence_ref(payload: &serde_json::Value) -> Option<String> {
    string_field(
        payload,
        &[
            "evidenceRef",
            "evidence_ref",
            "lastEvidenceRef",
            "last_evidence_ref",
            "recoveryRef",
            "recovery_ref",
        ],
    )
}

fn collect_payload_command_strings(value: &serde_json::Value, commands: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                collect_payload_command_strings(value, commands);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("command" | "cmd" | "script", serde_json::Value::String(command))
                        if !command.trim().is_empty() =>
                    {
                        commands.push(command.clone());
                    }
                    ("command" | "cmd", serde_json::Value::Array(parts)) => {
                        let command = parts
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" ");
                        if !command.trim().is_empty() {
                            commands.push(command);
                        }
                    }
                    _ => collect_payload_command_strings(value, commands),
                }
            }
        }
        _ => {}
    }
}

pub(super) fn string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for key in keys {
                if let Some(value) = map.get(*key).and_then(serde_json::Value::as_str) {
                    return Some(value.to_string());
                }
            }
            for value in map.values() {
                if let Some(found) = string_field(value, keys) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(values) => {
            values.iter().find_map(|value| string_field(value, keys))
        }
        _ => None,
    }
}
