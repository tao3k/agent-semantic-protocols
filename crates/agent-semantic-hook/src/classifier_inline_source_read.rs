use crate::protocol::normalize_source_route_selector;

pub(super) fn source_read_paths(command: &str, tokens: &[String]) -> Vec<String> {
    if !is_supported_interpreter(command, tokens) || !source_read_api(command) {
        return Vec::new();
    }
    quoted_path_literals(command)
}

fn is_supported_interpreter(command: &str, tokens: &[String]) -> bool {
    tokens.iter().any(|token| {
        is_python_interpreter_command(token) || is_javascript_interpreter_command(token)
    }) || command_has_inline_interpreter(command)
}

fn command_has_inline_interpreter(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    [
        "node -e",
        "node --eval",
        "nodejs -e",
        "python -c",
        "python3 -c",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_python_interpreter_command(token: &str) -> bool {
    let name = token.rsplit('/').next().unwrap_or(token);
    name == "python"
        || name == "python3"
        || name == "py"
        || name
            .strip_prefix("python3.")
            .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_digit() || ch == '.'))
}

fn is_javascript_interpreter_command(token: &str) -> bool {
    let name = token.rsplit('/').next().unwrap_or(token);
    name == "node"
        || name == "nodejs"
        || name
            .strip_prefix("node")
            .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_digit() || ch == '.'))
}

fn source_read_api(command: &str) -> bool {
    python_source_read_api(command) || javascript_source_read_api(command)
}

fn python_source_read_api(command: &str) -> bool {
    command.contains(".read_text(")
        || command.contains(".read_bytes(")
        || (command.contains("open(") && command.contains(".read("))
}

fn javascript_source_read_api(command: &str) -> bool {
    command.contains("readFileSync(")
        || command.contains(".readFile(")
        || command.contains("readFile(")
}

fn quoted_path_literals(command: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\'' && ch != '"' {
            continue;
        }
        let quote = ch;
        let mut literal = String::new();
        let mut escaped = false;
        for current in chars.by_ref() {
            if escaped {
                literal.push(current);
                escaped = false;
                continue;
            }
            if current == '\\' {
                escaped = true;
                continue;
            }
            if current == quote {
                break;
            }
            literal.push(current);
        }
        let normalized = normalize_source_route_selector(&literal);
        if is_path_like_literal(normalized) {
            paths.push(normalized.to_string());
        }
        paths.extend(quoted_path_literals(&literal));
    }
    paths
}

fn is_path_like_literal(literal: &str) -> bool {
    !literal.starts_with('-')
        && !literal.chars().any(char::is_whitespace)
        && (literal.contains('/') || literal.contains('*'))
        && literal.contains('.')
}
