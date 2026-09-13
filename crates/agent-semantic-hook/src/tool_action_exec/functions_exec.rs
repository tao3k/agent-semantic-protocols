// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Extracts nested shell calls from the `functions.exec` JavaScript envelope.

use serde_json::Value;

use super::NestedToolAction;

pub(super) fn nested_code_actions(tool_name: &str, tool_input: &Value) -> Vec<NestedToolAction> {
    if tool_name != "functions.exec" {
        return Vec::new();
    }
    let Some(code) = functions_exec_source(tool_input) else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    let mut cursor = 0;
    while let Some(call_start) = next_exec_command_call(code, cursor) {
        let Some(object_start) = call_object_start(code, call_start) else {
            cursor = call_start;
            continue;
        };
        let Some(object_end) = balanced_object_end(code, object_start) else {
            break;
        };
        if let Some((key, command)) = command_field(&code[object_start..object_end]) {
            let mut input = serde_json::Map::new();
            input.insert(key.to_owned(), Value::String(command));
            actions.push(NestedToolAction {
                tool_name: "exec_command".to_owned(),
                input: Value::Object(input),
            });
        }
        cursor = object_end;
    }
    cursor = 0;
    while let Some(call_start) = next_tool_call(code, cursor, "tools.apply_patch") {
        let Some((patch, call_end)) = call_string_argument(code, call_start) else {
            cursor = call_start;
            continue;
        };
        actions.push(NestedToolAction {
            tool_name: "apply_patch".to_owned(),
            input: Value::String(patch),
        });
        cursor = call_end;
    }
    actions
}

pub(super) fn functions_exec_source(tool_input: &Value) -> Option<&str> {
    tool_input
        .as_str()
        .or_else(|| tool_input.get("code").and_then(Value::as_str))
}

fn next_tool_call(code: &str, start: usize, callee: &str) -> Option<usize> {
    let bytes = code.as_bytes();
    let mut string = JsStringScan::default();
    let mut cursor = start;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if string.consume(byte) {
            cursor += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            string.open(byte);
            cursor += 1;
            continue;
        }
        if code[cursor..].starts_with(callee) {
            return Some(cursor + callee.len());
        }
        cursor += 1;
    }
    None
}

fn call_string_argument(code: &str, mut cursor: usize) -> Option<(String, usize)> {
    let bytes = code.as_bytes();
    skip_whitespace(bytes, &mut cursor);
    if bytes.get(cursor) != Some(&b'(') {
        return None;
    }
    cursor += 1;
    skip_whitespace(bytes, &mut cursor);
    if matches!(bytes.get(cursor), Some(b'\'' | b'"' | b'`')) {
        return decode_js_string_with_end(code, cursor);
    }
    let identifier_start = cursor;
    if !bytes
        .get(cursor)
        .is_some_and(|byte| is_identifier_start(*byte))
    {
        return None;
    }
    cursor += 1;
    while bytes.get(cursor).is_some_and(|byte| is_identifier(*byte)) {
        cursor += 1;
    }
    let identifier = &code[identifier_start..cursor];
    let patch = string_binding_before(code, identifier, identifier_start)?;
    Some((patch, cursor))
}

fn string_binding_before(code: &str, identifier: &str, before: usize) -> Option<String> {
    let prefix = code.get(..before)?;
    for declaration in ["const", "let", "var"] {
        let needle = format!("{declaration} {identifier}");
        let Some(binding_start) = prefix.rfind(&needle) else {
            continue;
        };
        let mut cursor = binding_start + needle.len();
        let bytes = code.as_bytes();
        skip_whitespace(bytes, &mut cursor);
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        skip_whitespace(bytes, &mut cursor);
        if let Some((value, _)) = decode_js_string_with_end(code, cursor) {
            return Some(value);
        }
    }
    None
}

fn next_exec_command_call(code: &str, start: usize) -> Option<usize> {
    const CALLEE: &str = "tools.exec_command";
    let bytes = code.as_bytes();
    let mut string = JsStringScan::default();
    let mut cursor = start;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if string.consume(byte) {
            cursor += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            string.open(byte);
            cursor += 1;
            continue;
        }
        if code[cursor..].starts_with(CALLEE) {
            return Some(cursor + CALLEE.len());
        }
        cursor += 1;
    }
    None
}

fn call_object_start(code: &str, mut cursor: usize) -> Option<usize> {
    let bytes = code.as_bytes();
    skip_whitespace(bytes, &mut cursor);
    if bytes.get(cursor) != Some(&b'(') {
        return None;
    }
    cursor += 1;
    skip_whitespace(bytes, &mut cursor);
    (bytes.get(cursor) == Some(&b'{')).then_some(cursor)
}

fn balanced_object_end(code: &str, object_start: usize) -> Option<usize> {
    let bytes = code.as_bytes();
    let mut depth = 0usize;
    let mut string = JsStringScan::default();
    for (index, byte) in bytes.iter().copied().enumerate().skip(object_start) {
        if string.consume(byte) {
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => string.open(byte),
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn command_field(object: &str) -> Option<(&'static str, String)> {
    let bytes = object.as_bytes();
    let mut cursor = 1usize;
    let mut depth = 1usize;
    let mut string = JsStringScan::default();
    while cursor + 1 < bytes.len() {
        let byte = bytes[cursor];
        if string.consume(byte) {
            cursor += 1;
            continue;
        }
        match byte {
            b'\'' | b'"' | b'`' => string.open(byte),
            b'{' | b'[' | b'(' => depth += 1,
            b'}' | b']' | b')' => depth = depth.saturating_sub(1),
            _ if depth == 1 && is_identifier_start(byte) => {
                if let Some(field) = read_command_field(object, &mut cursor) {
                    return Some(field);
                }
                continue;
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn read_command_field(object: &str, cursor: &mut usize) -> Option<(&'static str, String)> {
    let bytes = object.as_bytes();
    let key_start = *cursor;
    *cursor += 1;
    while bytes.get(*cursor).is_some_and(|byte| is_identifier(*byte)) {
        *cursor += 1;
    }
    let field = match &object[key_start..*cursor] {
        "cmd" => "cmd",
        "command" => "command",
        _ => return None,
    };
    skip_whitespace(bytes, cursor);
    if bytes.get(*cursor) != Some(&b':') {
        return None;
    }
    *cursor += 1;
    skip_whitespace(bytes, cursor);
    decode_js_string_at(object, *cursor).map(|command| (field, command))
}

fn decode_js_string_at(source: &str, start: usize) -> Option<String> {
    decode_js_string_with_end(source, start).map(|(decoded, _)| decoded)
}

fn decode_js_string_with_end(source: &str, start: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    let quote = *bytes.get(start)?;
    if !matches!(quote, b'\'' | b'"' | b'`') {
        return None;
    }
    let mut cursor = start + 1;
    let mut decoded = String::new();
    while cursor < bytes.len() {
        let character = source[cursor..].chars().next()?;
        if character == char::from(quote) {
            return Some((decoded, cursor + character.len_utf8()));
        }
        if quote == b'`' && source[cursor..].starts_with("${") {
            return None;
        }
        if character == '\\' {
            cursor += 1;
            let escaped = *bytes.get(cursor)?;
            decoded.push(match escaped {
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'\\' => '\\',
                b'\'' => '\'',
                b'"' => '"',
                b'`' => '`',
                _ => return None,
            });
            cursor += 1;
        } else {
            decoded.push(character);
            cursor += character.len_utf8();
        }
    }
    None
}

fn skip_whitespace(bytes: &[u8], cursor: &mut usize) {
    while bytes.get(*cursor).is_some_and(u8::is_ascii_whitespace) {
        *cursor += 1;
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[derive(Default)]
struct JsStringScan {
    quote: Option<u8>,
    escaped: bool,
}

impl JsStringScan {
    fn open(&mut self, quote: u8) {
        self.quote = Some(quote);
    }

    fn consume(&mut self, byte: u8) -> bool {
        let Some(quote) = self.quote else {
            return false;
        };
        if self.escaped {
            self.escaped = false;
        } else if byte == b'\\' {
            self.escaped = true;
        } else if byte == quote {
            self.quote = None;
        }
        true
    }
}
