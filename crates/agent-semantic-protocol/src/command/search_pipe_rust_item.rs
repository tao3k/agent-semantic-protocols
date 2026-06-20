//! Rust item-line helpers for search frontier receipts.

pub(super) fn rust_item_symbol_for_line(line: &[u8]) -> Option<String> {
    let line = String::from_utf8_lossy(line);
    let tokens = rust_identifier_tokens(&line);
    tokens
        .iter()
        .position(|token| rust_item_keyword(token))
        .and_then(|index| tokens.get(index + 1))
        .filter(|symbol| !rust_item_keyword(symbol))
        .cloned()
}

fn rust_identifier_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in line.chars() {
        if character == '_' || character.is_ascii_alphanumeric() {
            current.push(character);
            continue;
        }
        push_rust_identifier_token(&mut tokens, &mut current);
    }
    push_rust_identifier_token(&mut tokens, &mut current);
    tokens
}

fn push_rust_identifier_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty()
        && current
            .chars()
            .next()
            .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
    {
        tokens.push(current.clone());
    }
    current.clear();
}

fn rust_item_keyword(token: &str) -> bool {
    matches!(
        token,
        "struct" | "enum" | "trait" | "type" | "mod" | "const" | "static" | "fn"
    )
}
