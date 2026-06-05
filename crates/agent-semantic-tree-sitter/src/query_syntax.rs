//! Grammarless tree-sitter query ABI planning.
//!
//! This compiler validates the S-expression query surface and extracts the
//! portable pieces a native provider needs for tree-sitter-compatible capture
//! projection. It intentionally does not require a grammar `Language`.

use std::collections::BTreeSet;

/// Grammarless ABI plan extracted from tree-sitter-compatible query source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxQueryAbiPlan {
    pub patterns: Vec<SyntaxQueryAbiPattern>,
    pub captures: Vec<String>,
    pub node_types: Vec<String>,
    pub fields: Vec<String>,
    pub predicates: Vec<SyntaxQueryAbiPredicate>,
}

impl SyntaxQueryAbiPlan {
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

/// Predicate operator extracted from a tree-sitter-compatible query.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SyntaxQueryPredicateOp {
    Eq,
    AnyEq,
    AnyOf,
    Match,
    AnyMatch,
    NotEq,
    NotMatch,
}

impl SyntaxQueryPredicateOp {
    #[must_use]
    pub fn as_abi_str(&self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::AnyEq => "any-eq",
            Self::AnyOf => "any-of",
            Self::Match => "match",
            Self::AnyMatch => "any-match",
            Self::NotEq => "not-eq",
            Self::NotMatch => "not-match",
        }
    }
}

/// Predicate operand extracted from a tree-sitter-compatible query.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SyntaxQueryPredicateValue {
    String(String),
    Capture(String),
}

/// Predicate ABI fact extracted from one tree-sitter query predicate form.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SyntaxQueryAbiPredicate {
    pub op: SyntaxQueryPredicateOp,
    pub capture: String,
    pub values: Vec<SyntaxQueryPredicateValue>,
}

/// Per-pattern ABI facts extracted from one top-level query pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxQueryAbiPattern {
    pub index: usize,
    pub captures: Vec<String>,
    pub node_types: Vec<String>,
    pub fields: Vec<String>,
}

/// Error returned when grammarless query ABI planning rejects a source string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxQueryAbiError {
    pub message: String,
}

impl std::fmt::Display for SyntaxQueryAbiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SyntaxQueryAbiError {}

/// Compile tree-sitter-compatible query source into a grammarless ABI plan.
pub fn compile_query_abi_source(source: &str) -> Result<SyntaxQueryAbiPlan, SyntaxQueryAbiError> {
    let tokens = tokenize_query(source)?;
    AbiParser::new(tokens).parse()
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    LParen,
    RParen,
    LBracket,
    RBracket,
    Ident(String),
    Field(String),
    Capture(String),
    StringLiteral(String),
    Quantifier,
}

fn tokenize_query(source: &str) -> Result<Vec<Token>, SyntaxQueryAbiError> {
    let chars = source.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        let character = chars[index];
        match character {
            character if character.is_whitespace() => index += 1,
            ';' => {
                index += 1;
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
            }
            '(' => {
                tokens.push(Token::LParen);
                index += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                index += 1;
            }
            '[' => {
                tokens.push(Token::LBracket);
                index += 1;
            }
            ']' => {
                tokens.push(Token::RBracket);
                index += 1;
            }
            '"' => {
                let (literal, next) = read_string_literal(&chars, index)?;
                index = next;
                tokens.push(Token::StringLiteral(literal));
            }
            '@' => {
                let (capture, next) = read_atom(&chars, index + 1);
                let capture = trim_capture_quantifier(&capture);
                if capture.is_empty() {
                    return Err(error("empty capture name"));
                }
                tokens.push(Token::Capture(capture.to_string()));
                index = next;
            }
            '?' | '+' | '*' => {
                tokens.push(Token::Quantifier);
                index += 1;
            }
            _ => {
                let (atom, next) = read_atom(&chars, index);
                if atom.is_empty() {
                    return Err(error(format!("unexpected character `{character}`")));
                }
                if let Some(field) = atom.strip_suffix(':')
                    && !field.is_empty()
                {
                    tokens.push(Token::Field(field.to_string()));
                } else {
                    tokens.push(Token::Ident(atom.to_string()));
                }
                index = next;
            }
        }
    }
    Ok(tokens)
}

fn read_atom(chars: &[char], start: usize) -> (String, usize) {
    let mut end = start;
    while end < chars.len() && !is_atom_delimiter(chars[end]) {
        end += 1;
    }
    (chars[start..end].iter().collect(), end)
}

fn is_atom_delimiter(character: char) -> bool {
    character.is_whitespace() || matches!(character, '(' | ')' | '[' | ']' | '"' | ';')
}

fn read_string_literal(
    chars: &[char],
    start: usize,
) -> Result<(String, usize), SyntaxQueryAbiError> {
    let mut index = start + 1;
    let mut escaped = false;
    let mut literal = String::new();
    while index < chars.len() {
        let character = chars[index];
        if escaped {
            literal.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Ok((literal, index + 1));
        } else {
            literal.push(character);
        }
        index += 1;
    }
    Err(error("unterminated string literal"))
}

fn trim_capture_quantifier(capture: &str) -> &str {
    capture
        .strip_suffix('?')
        .or_else(|| capture.strip_suffix('+'))
        .or_else(|| capture.strip_suffix('*'))
        .unwrap_or(capture)
}

#[derive(Clone, Debug)]
struct FormContext {
    kind: FormKind,
    expects_head: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FormKind {
    Paren,
    Bracket,
}

#[derive(Clone, Debug)]
struct PatternBuilder {
    index: usize,
    captures: BTreeSet<String>,
    node_types: BTreeSet<String>,
    fields: BTreeSet<String>,
}

impl PatternBuilder {
    fn new(index: usize) -> Self {
        Self {
            index,
            captures: BTreeSet::new(),
            node_types: BTreeSet::new(),
            fields: BTreeSet::new(),
        }
    }

    fn finish(self) -> Result<SyntaxQueryAbiPattern, SyntaxQueryAbiError> {
        if self.captures.is_empty() && self.node_types.is_empty() {
            return Err(error(format!("empty query pattern {}", self.index)));
        }
        Ok(SyntaxQueryAbiPattern {
            index: self.index,
            captures: self.captures.into_iter().collect(),
            node_types: self.node_types.into_iter().collect(),
            fields: self.fields.into_iter().collect(),
        })
    }
}

struct AbiParser {
    tokens: Vec<Token>,
    stack: Vec<FormContext>,
    current: Option<PatternBuilder>,
    patterns: Vec<SyntaxQueryAbiPattern>,
}

impl AbiParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            stack: Vec::new(),
            current: None,
            patterns: Vec::new(),
        }
    }

    fn parse(mut self) -> Result<SyntaxQueryAbiPlan, SyntaxQueryAbiError> {
        if self.tokens.is_empty() {
            return Err(error("empty query source"));
        }
        let tokens = std::mem::take(&mut self.tokens);
        let predicates = query_predicates(&tokens);
        for token in tokens {
            self.accept(token)?;
        }
        if !self.stack.is_empty() {
            return Err(error("unclosed query pattern"));
        }
        self.finish_current_pattern()?;
        if self.patterns.is_empty() {
            return Err(error("query source contains no patterns"));
        }
        let captures = union_sorted(self.patterns.iter().flat_map(|pattern| &pattern.captures));
        let node_types = union_sorted(self.patterns.iter().flat_map(|pattern| &pattern.node_types));
        let fields = union_sorted(self.patterns.iter().flat_map(|pattern| &pattern.fields));
        Ok(SyntaxQueryAbiPlan {
            patterns: self.patterns,
            captures,
            node_types,
            fields,
            predicates,
        })
    }

    fn accept(&mut self, token: Token) -> Result<(), SyntaxQueryAbiError> {
        match token {
            Token::LParen => {
                self.open_pattern_if_needed()?;
                self.stack.push(FormContext {
                    kind: FormKind::Paren,
                    expects_head: true,
                });
            }
            Token::LBracket => {
                self.open_pattern_if_needed()?;
                self.stack.push(FormContext {
                    kind: FormKind::Bracket,
                    expects_head: false,
                });
            }
            Token::RParen => self.close_form(FormKind::Paren)?,
            Token::RBracket => self.close_form(FormKind::Bracket)?,
            Token::Field(field) => {
                if self.stack.is_empty() {
                    return Err(error(format!(
                        "field `{field}` appears outside a query form"
                    )));
                }
                let current = self.current.as_mut().ok_or_else(|| {
                    error(format!("field `{field}` appears outside a query pattern"))
                })?;
                current.fields.insert(field);
                self.mark_head_consumed();
            }
            Token::Capture(capture) => {
                let current = self.current.as_mut().ok_or_else(|| {
                    error(format!(
                        "capture `{capture}` appears outside a query pattern"
                    ))
                })?;
                current.captures.insert(capture);
                self.mark_head_consumed();
            }
            Token::Ident(identifier) => {
                if self.consume_node_head(&identifier)
                    && identifier != "_"
                    && !identifier.starts_with('#')
                    && let Some(current) = self.current.as_mut()
                {
                    current.node_types.insert(identifier);
                }
            }
            Token::StringLiteral(_) | Token::Quantifier => self.mark_head_consumed(),
        }
        Ok(())
    }

    fn open_pattern_if_needed(&mut self) -> Result<(), SyntaxQueryAbiError> {
        if self.stack.is_empty() {
            self.finish_current_pattern()?;
            self.current = Some(PatternBuilder::new(self.patterns.len()));
        }
        Ok(())
    }

    fn close_form(&mut self, expected: FormKind) -> Result<(), SyntaxQueryAbiError> {
        let context = self
            .stack
            .pop()
            .ok_or_else(|| error("unexpected closing delimiter"))?;
        if context.kind != expected {
            return Err(error("mismatched query delimiters"));
        }
        Ok(())
    }

    fn finish_current_pattern(&mut self) -> Result<(), SyntaxQueryAbiError> {
        if let Some(current) = self.current.take() {
            self.patterns.push(current.finish()?);
        }
        Ok(())
    }

    fn consume_node_head(&mut self, identifier: &str) -> bool {
        let Some(context) = self.stack.last_mut() else {
            return false;
        };
        if !context.expects_head {
            return false;
        }
        context.expects_head = false;
        !identifier.starts_with('#')
    }

    fn mark_head_consumed(&mut self) {
        if let Some(context) = self.stack.last_mut() {
            context.expects_head = false;
        }
    }
}

fn query_predicates(tokens: &[Token]) -> Vec<SyntaxQueryAbiPredicate> {
    let mut predicates = BTreeSet::new();
    let mut index = 0usize;
    while index + 3 < tokens.len() {
        let predicate = match (&tokens[index], &tokens[index + 1], &tokens[index + 2]) {
            (Token::LParen, Token::Ident(predicate), Token::Capture(capture)) => {
                predicate_op(predicate).map(|op| (op, capture.clone()))
            }
            _ => None,
        };
        if let Some((op, capture)) = predicate {
            let mut values = Vec::new();
            let mut cursor = index + 3;
            while cursor < tokens.len() && !matches!(tokens[cursor], Token::RParen) {
                match &tokens[cursor] {
                    Token::StringLiteral(value) if !value.is_empty() => {
                        values.push(SyntaxQueryPredicateValue::String(value.clone()));
                    }
                    Token::Capture(value) if !value.is_empty() => {
                        values.push(SyntaxQueryPredicateValue::Capture(value.clone()));
                    }
                    _ => {}
                }
                cursor += 1;
            }
            values.sort();
            values.dedup();
            if !values.is_empty() {
                predicates.insert(SyntaxQueryAbiPredicate {
                    op,
                    capture,
                    values,
                });
            }
            index = cursor;
        }
        index += 1;
    }
    predicates.into_iter().collect()
}

fn predicate_op(predicate: &str) -> Option<SyntaxQueryPredicateOp> {
    match predicate {
        "#eq?" => Some(SyntaxQueryPredicateOp::Eq),
        "#any-eq?" => Some(SyntaxQueryPredicateOp::AnyEq),
        "#any-of?" => Some(SyntaxQueryPredicateOp::AnyOf),
        "#match?" => Some(SyntaxQueryPredicateOp::Match),
        "#any-match?" => Some(SyntaxQueryPredicateOp::AnyMatch),
        "#not-eq?" => Some(SyntaxQueryPredicateOp::NotEq),
        "#not-match?" => Some(SyntaxQueryPredicateOp::NotMatch),
        _ => None,
    }
}

fn union_sorted<'a>(values: impl Iterator<Item = &'a String>) -> Vec<String> {
    values
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn error(message: impl Into<String>) -> SyntaxQueryAbiError {
    SyntaxQueryAbiError {
        message: message.into(),
    }
}
