use serde_json::{Value, json};

pub fn stable_graph_node_id(kind: &str, value: &str) -> String {
    let mut rendered = String::with_capacity(kind.len() + value.len() + 1);
    rendered.push_str(kind);
    rendered.push(':');
    for character in value.chars() {
        if character == '_' || character == '-' || character == '/' || character == '.' {
            rendered.push(character);
        } else if character.is_ascii_alphanumeric() {
            rendered.push(character.to_ascii_lowercase());
        } else {
            rendered.push('-');
        }
    }
    while rendered.ends_with('-') {
        rendered.pop();
    }
    if rendered.len() == kind.len() + 1 {
        rendered.push_str("node");
    }
    rendered
}

pub fn owner_path_graph_nodes(owners: &[String]) -> Vec<Value> {
    owners
        .iter()
        .map(|owner| {
            json!({
                "id": stable_graph_node_id("owner", owner),
                "kind": "owner",
                "role": "path",
                "value": owner,
                "action": "owner",
                "path": owner
            })
        })
        .collect()
}
