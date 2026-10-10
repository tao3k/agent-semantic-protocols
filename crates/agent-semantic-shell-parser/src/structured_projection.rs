// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
            let value_count = usize::from(*value_count);
            if words.by_ref().take(value_count).count() != value_count {
                return StructuredFilterClassification::Invalid;
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
    BoundedPathFilterParser::new(filter, max_slice_items).classify()
}

struct BoundedPathFilterParser<'a> {
    filter: &'a str,
    cursor: usize,
    max_slice_items: usize,
    segments: Vec<BoundedPathSegment>,
}

impl<'a> BoundedPathFilterParser<'a> {
    fn new(filter: &'a str, max_slice_items: usize) -> Self {
        Self {
            filter,
            cursor: 1,
            max_slice_items,
            segments: Vec::new(),
        }
    }

    fn classify(mut self) -> StructuredFilterClassification {
        while self.cursor < self.filter.len() {
            if let Err(classification) = self.parse_next_segment() {
                return classification;
            }
        }
        if self.segments.is_empty() {
            StructuredFilterClassification::Identity
        } else {
            StructuredFilterClassification::BoundedPath {
                segments: self.segments,
                source_operands: Vec::new(),
            }
        }
    }

    fn parse_next_segment(&mut self) -> Result<(), StructuredFilterClassification> {
        match self.bytes()[self.cursor] {
            b'.' => self.parse_dotted_field(),
            b'[' => self.parse_bracket_segment(),
            byte if is_identifier_start(byte) => self.parse_identifier_field(),
            b'|' | b',' | b'{' | b'}' | b'(' | b')' | b'?' | b'=' | b';' => {
                Err(StructuredFilterClassification::Compound)
            }
            _ => Err(StructuredFilterClassification::Invalid),
        }
    }

    fn parse_dotted_field(&mut self) -> Result<(), StructuredFilterClassification> {
        if self.bytes().get(self.cursor + 1) == Some(&b'.') || self.cursor == 1 {
            return Err(StructuredFilterClassification::RecursiveDescent);
        }
        self.cursor += 1;
        let start = self.cursor;
        self.consume_identifier();
        if start == self.cursor {
            return Err(classify_unbounded_token(
                self.bytes().get(self.cursor).copied(),
            ));
        }
        self.push_field(start);
        Ok(())
    }

    fn parse_identifier_field(&mut self) -> Result<(), StructuredFilterClassification> {
        let start = self.cursor;
        self.cursor += 1;
        self.consume_identifier();
        self.push_field(start);
        Ok(())
    }

    fn parse_bracket_segment(&mut self) -> Result<(), StructuredFilterClassification> {
        self.cursor += 1;
        if self.bytes().get(self.cursor) == Some(&b']') {
            return Err(StructuredFilterClassification::ArrayIteration);
        }
        if self.bytes().get(self.cursor) == Some(&b'\"') {
            self.parse_quoted_field()?;
        } else {
            self.parse_numeric_segment()?;
        }
        if self.bytes().get(self.cursor) != Some(&b']') {
            return Err(StructuredFilterClassification::Compound);
        }
        self.cursor += 1;
        Ok(())
    }

    fn parse_quoted_field(&mut self) -> Result<(), StructuredFilterClassification> {
        self.cursor += 1;
        let start = self.cursor;
        while self.cursor < self.filter.len() && self.bytes()[self.cursor] != b'\"' {
            if self.bytes()[self.cursor] == b'\\' {
                return Err(StructuredFilterClassification::Invalid);
            }
            self.cursor += 1;
        }
        if self.cursor == start || self.bytes().get(self.cursor) != Some(&b'\"') {
            return Err(StructuredFilterClassification::Invalid);
        }
        self.push_field(start);
        self.cursor += 1;
        Ok(())
    }

    fn parse_numeric_segment(&mut self) -> Result<(), StructuredFilterClassification> {
        let start = self.cursor;
        self.consume_digits();
        if self.bytes().get(self.cursor) == Some(&b':') {
            self.parse_slice(start)
        } else if start == self.cursor {
            Err(StructuredFilterClassification::Compound)
        } else {
            let index = self.filter[start..self.cursor]
                .parse()
                .map_err(|_| StructuredFilterClassification::Invalid)?;
            self.segments.push(BoundedPathSegment::Index(index));
            Ok(())
        }
    }

    fn parse_slice(&mut self, start: usize) -> Result<(), StructuredFilterClassification> {
        let slice_start = if start == self.cursor {
            0
        } else {
            self.filter[start..self.cursor]
                .parse()
                .map_err(|_| StructuredFilterClassification::Invalid)?
        };
        self.cursor += 1;
        let end_start = self.cursor;
        self.consume_digits();
        if end_start == self.cursor {
            return Err(StructuredFilterClassification::Compound);
        }
        let slice_end = self.filter[end_start..self.cursor]
            .parse::<usize>()
            .map_err(|_| StructuredFilterClassification::Invalid)?;
        if slice_end.saturating_sub(slice_start) > self.max_slice_items {
            return Err(StructuredFilterClassification::Compound);
        }
        self.segments.push(BoundedPathSegment::Slice {
            start: slice_start,
            end: slice_end,
        });
        Ok(())
    }

    fn consume_identifier(&mut self) {
        while self.cursor < self.filter.len() && is_identifier_continue(self.bytes()[self.cursor]) {
            self.cursor += 1;
        }
    }

    fn consume_digits(&mut self) {
        while self.cursor < self.filter.len() && self.bytes()[self.cursor].is_ascii_digit() {
            self.cursor += 1;
        }
    }

    fn push_field(&mut self, start: usize) {
        self.segments.push(BoundedPathSegment::Field(
            self.filter[start..self.cursor].to_string(),
        ));
    }

    fn bytes(&self) -> &[u8] {
        self.filter.as_bytes()
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
