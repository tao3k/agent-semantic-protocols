//! Bounded structured-filter command classification.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
/// One bounded structured-filter path segment.
pub enum BoundedPathSegment {
    Field(String),
    Index(usize),
    Slice { start: usize, end: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Classification of a structured-filter command.
pub enum StructuredFilterClassification {
    BoundedPath {
        segments: Vec<BoundedPathSegment>,
        source_operands: Vec<String>,
    },
    BoundedScalarPredicate {
        predicate: String,
        source_operands: Vec<String>,
    },
    Identity,
    RecursiveDescent,
    ArrayIteration,
    Compound,
    Invalid,
    NotApplicable,
}

/// Configured command grammar for one bounded structured-filter input.
pub struct BoundedPathCommandSpec<'a> {
    pub binary: &'a str,
    pub optional_subcommand_any: &'a [String],
    pub option_any: &'a [String],
    pub option_value_arity: &'a BTreeMap<String, u8>,
    pub max_slice_items: usize,
}

/// Classify one bounded-path command without accepting compound shell stages.
pub fn classify_single_bounded_path_command(
    command: &str,
    spec: BoundedPathCommandSpec<'_>,
) -> StructuredFilterClassification {
    classify_single_bounded_path_command_impl(command, spec)
}

/// Classify a command from the shared Bash parser's normalized argv.
///
/// Hook decision shards already own this projection and must not parse the
/// original shell source a second time on the one-shot path.
pub fn classify_single_bounded_path_tokens(
    tokens: &[String],
    spec: BoundedPathCommandSpec<'_>,
) -> StructuredFilterClassification {
    if tokens
        .iter()
        .any(|token| crate::bash_parser::is_separator(token))
    {
        return StructuredFilterClassification::Compound;
    }
    let mut matching = tokens.iter().enumerate().filter_map(|(index, token)| {
        (token.rsplit('/').next() == Some(spec.binary)).then_some(index)
    });
    let Some(index) = matching.next() else {
        return StructuredFilterClassification::NotApplicable;
    };
    if matching.next().is_some() {
        return StructuredFilterClassification::Compound;
    }
    classify_bounded_path_words(&tokens[index..], &spec)
}

fn classify_single_bounded_path_command_impl(
    command: &str,
    spec: BoundedPathCommandSpec<'_>,
) -> StructuredFilterClassification {
    let Ok(stages) = crate::parse_bash_command_candidates(command) else {
        return StructuredFilterClassification::Invalid;
    };
    if stages.iter().any(|stage| {
        stage.words().len() == 1 && crate::bash_parser::is_separator(stage.words()[0].as_str())
    }) {
        return StructuredFilterClassification::Compound;
    }
    let matching = stages
        .iter()
        .flat_map(|stage| {
            stage
                .words()
                .iter()
                .enumerate()
                .filter_map(|(index, word)| {
                    (word.rsplit('/').next() == Some(spec.binary))
                        .then_some(&stage.words()[index..])
                })
        })
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [words] => classify_bounded_path_words(words, &spec),
        [] => StructuredFilterClassification::NotApplicable,
        _ => StructuredFilterClassification::Compound,
    }
}

fn classify_bounded_path_words(
    words: &[String],
    spec: &BoundedPathCommandSpec<'_>,
) -> StructuredFilterClassification {
    let Some(executable) = words.first() else {
        return StructuredFilterClassification::Invalid;
    };
    if executable.rsplit('/').next() != Some(spec.binary) {
        return StructuredFilterClassification::Invalid;
    }

    let mut words = words.iter().skip(1).peekable();
    if words.peek().is_some_and(|word| {
        spec.optional_subcommand_any
            .iter()
            .any(|subcommand| subcommand == *word)
    }) {
        let _ = words.next();
    }

    let filter = loop {
        let Some(word) = words.next() else {
            return StructuredFilterClassification::Invalid;
        };
        if word == "--" {
            let Some(filter) = words.next() else {
                return StructuredFilterClassification::Invalid;
            };
            break filter;
        }
        if let Some(value_count) = spec.option_value_arity.get(word) {
            for _ in 0..*value_count {
                if words.next().is_none() {
                    return StructuredFilterClassification::Invalid;
                }
            }
            continue;
        }
        if spec.option_any.iter().any(|option| option == word) {
            continue;
        }
        if word.starts_with('-') {
            return StructuredFilterClassification::Invalid;
        }
        break word;
    };

    let source_operands = words
        .filter(|word| word.as_str() != "--")
        .cloned()
        .collect::<Vec<_>>();
    if source_operands.len() != 1 {
        return StructuredFilterClassification::Compound;
    }
    match classify_bounded_path_filter_with_limit(filter, spec.max_slice_items) {
        StructuredFilterClassification::BoundedPath { segments, .. } => {
            StructuredFilterClassification::BoundedPath {
                segments,
                source_operands,
            }
        }
        StructuredFilterClassification::BoundedScalarPredicate { predicate, .. } => {
            StructuredFilterClassification::BoundedScalarPredicate {
                predicate,
                source_operands,
            }
        }
        classification => classification,
    }
}

/// Classify a structured-filter expression without executing it.
pub fn classify_bounded_path_filter(filter: &str) -> StructuredFilterClassification {
    classify_bounded_path_filter_with_limit(filter, 0)
}

fn classify_bounded_path_filter_with_limit(
    filter: &str,
    max_slice_items: usize,
) -> StructuredFilterClassification {
    let filter = filter.trim();
    if filter == "." {
        return StructuredFilterClassification::Identity;
    }
    if let Some((left, right)) = filter.split_once("==")
        && left.trim() == "type"
        && matches!(
            right.trim(),
            "\"object\"" | "\"array\"" | "\"string\"" | "\"number\"" | "\"boolean\"" | "\"null\""
        )
    {
        return StructuredFilterClassification::BoundedScalarPredicate {
            predicate: filter.to_owned(),
            source_operands: Vec::new(),
        };
    }
    if !filter.starts_with('.') {
        return StructuredFilterClassification::Invalid;
    }

    let bytes = filter.as_bytes();
    let mut cursor = 1;
    let mut segments = Vec::new();
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'.' => {
                if bytes.get(cursor + 1) == Some(&b'.') || cursor == 1 {
                    return StructuredFilterClassification::RecursiveDescent;
                }
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && is_identifier_continue(bytes[cursor]) {
                    cursor += 1;
                }
                if start == cursor {
                    return classify_unbounded_token(bytes.get(cursor).copied());
                }
                segments.push(BoundedPathSegment::Field(filter[start..cursor].to_string()));
            }
            b'[' => {
                cursor += 1;
                if bytes.get(cursor) == Some(&b']') {
                    return StructuredFilterClassification::ArrayIteration;
                }
                if bytes.get(cursor) == Some(&b'\"') {
                    cursor += 1;
                    let start = cursor;
                    while cursor < bytes.len() && bytes[cursor] != b'\"' {
                        if bytes[cursor] == b'\\' {
                            return StructuredFilterClassification::Invalid;
                        }
                        cursor += 1;
                    }
                    if cursor == start || bytes.get(cursor) != Some(&b'\"') {
                        return StructuredFilterClassification::Invalid;
                    }
                    segments.push(BoundedPathSegment::Field(filter[start..cursor].to_string()));
                    cursor += 1;
                } else {
                    let start = cursor;
                    while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                        cursor += 1;
                    }
                    if bytes.get(cursor) == Some(&b':') {
                        let slice_start = if start == cursor {
                            0
                        } else {
                            let Ok(value) = filter[start..cursor].parse() else {
                                return StructuredFilterClassification::Invalid;
                            };
                            value
                        };
                        cursor += 1;
                        let end_start = cursor;
                        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                            cursor += 1;
                        }
                        if end_start == cursor {
                            return StructuredFilterClassification::Compound;
                        }
                        let Ok(slice_end) = filter[end_start..cursor].parse::<usize>() else {
                            return StructuredFilterClassification::Invalid;
                        };
                        if slice_end.saturating_sub(slice_start) > max_slice_items {
                            return StructuredFilterClassification::Compound;
                        }
                        segments.push(BoundedPathSegment::Slice {
                            start: slice_start,
                            end: slice_end,
                        });
                    } else if start == cursor {
                        return StructuredFilterClassification::Compound;
                    } else {
                        let Ok(index) = filter[start..cursor].parse() else {
                            return StructuredFilterClassification::Invalid;
                        };
                        segments.push(BoundedPathSegment::Index(index));
                    }
                }
                if bytes.get(cursor) != Some(&b']') {
                    return StructuredFilterClassification::Compound;
                }
                cursor += 1;
            }
            byte if is_identifier_start(byte) => {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len() && is_identifier_continue(bytes[cursor]) {
                    cursor += 1;
                }
                segments.push(BoundedPathSegment::Field(filter[start..cursor].to_string()));
            }
            b'|' | b',' | b'{' | b'}' | b'(' | b')' | b'?' | b'=' | b';' => {
                return StructuredFilterClassification::Compound;
            }
            _ => return StructuredFilterClassification::Invalid,
        }
    }

    if segments.is_empty() {
        StructuredFilterClassification::Identity
    } else {
        StructuredFilterClassification::BoundedPath {
            segments,
            source_operands: Vec::new(),
        }
    }
}

fn classify_unbounded_token(token: Option<u8>) -> StructuredFilterClassification {
    match token {
        Some(b'[') => StructuredFilterClassification::ArrayIteration,
        Some(b'.') => StructuredFilterClassification::RecursiveDescent,
        Some(b'|') | Some(b',') | Some(b'{') | Some(b'(') | Some(b'?') => {
            StructuredFilterClassification::Compound
        }
        _ => StructuredFilterClassification::Invalid,
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
#[path = "../tests/unit/structured_projection.rs"]
mod tests;
