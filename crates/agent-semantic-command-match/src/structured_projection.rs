use std::collections::BTreeMap;

use crate::CommandStageV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoundedPathSegmentV1 {
    Field(String),
    Index(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructuredFilterClassificationV1 {
    BoundedPath { segments: Vec<BoundedPathSegmentV1> },
    Identity,
    RecursiveDescent,
    ArrayIteration,
    Compound,
    Invalid,
}

pub struct BoundedPathCommandSpecV1<'a> {
    pub binary: &'a str,
    pub optional_subcommand_any: &'a [String],
    pub option_any: &'a [String],
    pub option_value_arity: &'a BTreeMap<String, u8>,
}

pub fn classify_single_bounded_path_command(
    command: &str,
    spec: BoundedPathCommandSpecV1<'_>,
) -> StructuredFilterClassificationV1 {
    let Ok(stages) = crate::parse_bash_command_candidates(command) else {
        return StructuredFilterClassificationV1::Invalid;
    };
    if stages.len() != 1 {
        return StructuredFilterClassificationV1::Compound;
    }
    classify_bounded_path_stage(&stages[0], spec)
}

fn classify_bounded_path_stage(
    stage: &CommandStageV1,
    spec: BoundedPathCommandSpecV1<'_>,
) -> StructuredFilterClassificationV1 {
    let Some(executable) = stage.executable() else {
        return StructuredFilterClassificationV1::Invalid;
    };
    if executable.rsplit('/').next() != Some(spec.binary) {
        return StructuredFilterClassificationV1::Invalid;
    }

    let mut words = stage.words().iter().skip(1).peekable();
    if words.peek().is_some_and(|word| {
        spec.optional_subcommand_any
            .iter()
            .any(|subcommand| subcommand == *word)
    }) {
        let _ = words.next();
    }

    let filter = loop {
        let Some(word) = words.next() else {
            return StructuredFilterClassificationV1::Invalid;
        };
        if word == "--" {
            let Some(filter) = words.next() else {
                return StructuredFilterClassificationV1::Invalid;
            };
            break filter;
        }
        if let Some(value_count) = spec.option_value_arity.get(word) {
            for _ in 0..*value_count {
                if words.next().is_none() {
                    return StructuredFilterClassificationV1::Invalid;
                }
            }
            continue;
        }
        if spec.option_any.iter().any(|option| option == word) {
            continue;
        }
        if word.starts_with('-') {
            return StructuredFilterClassificationV1::Invalid;
        }
        break word;
    };

    let remaining_operands = words.filter(|word| word.as_str() != "--").count();
    if remaining_operands != 1 {
        return StructuredFilterClassificationV1::Compound;
    }
    classify_bounded_path_filter(filter)
}

pub fn classify_bounded_path_filter(filter: &str) -> StructuredFilterClassificationV1 {
    let filter = filter.trim();
    if filter == "." {
        return StructuredFilterClassificationV1::Identity;
    }
    if !filter.starts_with('.') {
        return StructuredFilterClassificationV1::Invalid;
    }

    let bytes = filter.as_bytes();
    let mut cursor = 1;
    let mut segments = Vec::new();
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'.' => {
                if bytes.get(cursor + 1) == Some(&b'.') || cursor == 1 {
                    return StructuredFilterClassificationV1::RecursiveDescent;
                }
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && is_identifier_continue(bytes[cursor]) {
                    cursor += 1;
                }
                if start == cursor {
                    return classify_unbounded_token(bytes.get(cursor).copied());
                }
                segments.push(BoundedPathSegmentV1::Field(
                    filter[start..cursor].to_string(),
                ));
            }
            b'[' => {
                cursor += 1;
                if bytes.get(cursor) == Some(&b']') {
                    return StructuredFilterClassificationV1::ArrayIteration;
                }
                if bytes.get(cursor) == Some(&b'\"') {
                    cursor += 1;
                    let start = cursor;
                    while cursor < bytes.len() && bytes[cursor] != b'\"' {
                        if bytes[cursor] == b'\\' {
                            return StructuredFilterClassificationV1::Invalid;
                        }
                        cursor += 1;
                    }
                    if cursor == start || bytes.get(cursor) != Some(&b'\"') {
                        return StructuredFilterClassificationV1::Invalid;
                    }
                    segments.push(BoundedPathSegmentV1::Field(
                        filter[start..cursor].to_string(),
                    ));
                    cursor += 1;
                } else {
                    let start = cursor;
                    while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                        cursor += 1;
                    }
                    if start == cursor {
                        return StructuredFilterClassificationV1::Compound;
                    }
                    let Ok(index) = filter[start..cursor].parse() else {
                        return StructuredFilterClassificationV1::Invalid;
                    };
                    segments.push(BoundedPathSegmentV1::Index(index));
                }
                if bytes.get(cursor) != Some(&b']') {
                    return StructuredFilterClassificationV1::Compound;
                }
                cursor += 1;
            }
            byte if is_identifier_start(byte) => {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len() && is_identifier_continue(bytes[cursor]) {
                    cursor += 1;
                }
                segments.push(BoundedPathSegmentV1::Field(
                    filter[start..cursor].to_string(),
                ));
            }
            b'|' | b',' | b'{' | b'}' | b'(' | b')' | b'?' | b'=' | b';' => {
                return StructuredFilterClassificationV1::Compound;
            }
            _ => return StructuredFilterClassificationV1::Invalid,
        }
    }

    if segments.is_empty() {
        StructuredFilterClassificationV1::Identity
    } else {
        StructuredFilterClassificationV1::BoundedPath { segments }
    }
}

fn classify_unbounded_token(token: Option<u8>) -> StructuredFilterClassificationV1 {
    match token {
        Some(b'[') => StructuredFilterClassificationV1::ArrayIteration,
        Some(b'.') => StructuredFilterClassificationV1::RecursiveDescent,
        Some(b'|') | Some(b',') | Some(b'{') | Some(b'(') | Some(b'?') => {
            StructuredFilterClassificationV1::Compound
        }
        _ => StructuredFilterClassificationV1::Invalid,
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        BoundedPathCommandSpecV1, StructuredFilterClassificationV1,
        classify_single_bounded_path_command,
    };

    fn spec<'a>(
        binary: &'a str,
        subcommands: &'a [String],
        options: &'a [String],
        option_values: &'a BTreeMap<String, u8>,
    ) -> BoundedPathCommandSpecV1<'a> {
        BoundedPathCommandSpecV1 {
            binary,
            optional_subcommand_any: subcommands,
            option_any: options,
            option_value_arity: option_values,
        }
    }

    #[test]
    fn command_model_is_configured_instead_of_binary_hardcoded() {
        let no_subcommands = Vec::new();
        let options = vec!["--raw-output".to_string()];
        let mut option_values = BTreeMap::new();
        option_values.insert("--arg".to_string(), 2);
        assert!(matches!(
            classify_single_bounded_path_command(
                "project-json --arg scope workspace .package.name package.json",
                spec("project-json", &no_subcommands, &options, &option_values),
            ),
            StructuredFilterClassificationV1::BoundedPath { .. }
        ));
    }

    #[test]
    fn rejects_identity_multiple_inputs_and_multi_stage_commands() {
        let subcommands = vec!["eval".to_string(), "e".to_string()];
        let options = Vec::new();
        let option_values = BTreeMap::new();
        let configured = || spec("project-toml", &subcommands, &options, &option_values);
        assert_eq!(
            classify_single_bounded_path_command("project-toml eval . Cargo.toml", configured()),
            StructuredFilterClassificationV1::Identity
        );
        assert_eq!(
            classify_single_bounded_path_command(
                "project-toml .workspace Cargo.toml pyproject.toml",
                configured(),
            ),
            StructuredFilterClassificationV1::Compound
        );
        assert_eq!(
            classify_single_bounded_path_command(
                "project-toml .workspace Cargo.toml | sed Cargo.toml",
                configured(),
            ),
            StructuredFilterClassificationV1::Compound
        );
    }
}
