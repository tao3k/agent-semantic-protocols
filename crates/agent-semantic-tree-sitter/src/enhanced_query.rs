// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Hierarchy-preserving parser for the enhanced Tree-sitter Query source plane.
//!
//! This module parses the Query language itself. Provider capability admission
//! and lowering into resident facts happen at the Runtime boundary.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedQueryDocument {
    pub patterns: Vec<EnhancedQueryPattern>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedQueryPattern {
    pub index: usize,
    pub structure: EnhancedQueryExpression,
    pub predicates: Vec<EnhancedQueryPredicate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnhancedQueryExpression {
    NamedNode {
        name: String,
        quantifier: EnhancedQueryQuantifier,
        captures: Vec<String>,
        children: Vec<EnhancedQueryExpression>,
    },
    AnonymousNode {
        name: String,
        quantifier: EnhancedQueryQuantifier,
        captures: Vec<String>,
    },
    Field {
        name: String,
        value: Box<EnhancedQueryExpression>,
    },
    NegatedField(String),
    Alternation {
        quantifier: EnhancedQueryQuantifier,
        captures: Vec<String>,
        alternatives: Vec<EnhancedQueryExpression>,
    },
    Group {
        quantifier: EnhancedQueryQuantifier,
        captures: Vec<String>,
        terms: Vec<EnhancedQueryExpression>,
    },
    Anchor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnhancedQueryQuantifier {
    pub minimum: usize,
    pub maximum: Option<usize>,
}

impl EnhancedQueryQuantifier {
    pub const ONE: Self = Self {
        minimum: 1,
        maximum: Some(1),
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedQueryPredicate {
    pub name: String,
    pub kind: EnhancedQueryPredicateKind,
    pub operands: Vec<EnhancedQueryOperand>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnhancedQueryPredicateKind {
    Predicate,
    Directive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnhancedQueryOperand {
    Capture(String),
    String(String),
    Bare(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedQueryParseError {
    pub message: String,
}

impl std::fmt::Display for EnhancedQueryParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for EnhancedQueryParseError {}

pub fn parse_enhanced_query_source(
    source: &str,
) -> Result<EnhancedQueryDocument, EnhancedQueryParseError> {
    if source.trim().is_empty() {
        return Err(error("enhanced Query source is empty"));
    }
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_tsquery::LANGUAGE.into())
        .map_err(|cause| error(format!("load Tree-sitter Query grammar: {cause}")))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| error("Tree-sitter Query parser returned no tree"))?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(error(format!(
            "enhanced Query source is syntactically invalid: {}",
            root.to_sexp()
        )));
    }

    let mut patterns: Vec<EnhancedQueryPattern> = Vec::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() == "comment" {
            continue;
        }
        if node.kind() == "predicate" {
            let pattern = patterns.last_mut().ok_or_else(|| {
                error("enhanced Query predicate appears before its structural pattern")
            })?;
            pattern.predicates.push(parse_predicate(node, source)?);
            continue;
        }
        let mut predicates = Vec::new();
        let structure = parse_expression(node, source, &mut predicates)?;
        patterns.push(EnhancedQueryPattern {
            index: patterns.len(),
            structure,
            predicates,
        });
    }
    if patterns.is_empty() {
        return Err(error(
            "enhanced Query source contains no structural patterns",
        ));
    }
    Ok(EnhancedQueryDocument { patterns })
}

fn parse_expression(
    node: tree_sitter::Node<'_>,
    source: &str,
    predicates: &mut Vec<EnhancedQueryPredicate>,
) -> Result<EnhancedQueryExpression, EnhancedQueryParseError> {
    match node.kind() {
        "named_node" => {
            let name = required_field_text(node, "name", source)?;
            let quantifier = query_quantifier(node, source)?;
            let captures = query_captures(node, source)?;
            let mut children = Vec::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.id()
                    == node
                        .child_by_field_name("name")
                        .map(|value| value.id())
                        .unwrap_or(0)
                    || matches!(child.kind(), "(" | ")" | "quantifier" | "capture")
                {
                    continue;
                }
                if child.kind() == "predicate" {
                    predicates.push(parse_predicate(child, source)?);
                } else if child.kind() == "." {
                    children.push(EnhancedQueryExpression::Anchor);
                } else if child.is_named() {
                    children.push(parse_expression(child, source, predicates)?);
                }
            }
            Ok(EnhancedQueryExpression::NamedNode {
                name,
                quantifier,
                captures,
                children,
            })
        }
        "anonymous_node" => Ok(EnhancedQueryExpression::AnonymousNode {
            name: required_field_text(node, "name", source)?,
            quantifier: query_quantifier(node, source)?,
            captures: query_captures(node, source)?,
        }),
        "field_definition" => {
            let name_node = node
                .child_by_field_name("name")
                .ok_or_else(|| error("enhanced Query field lacks a name"))?;
            let name = node_text(name_node, source)?
                .trim_end_matches(':')
                .to_owned();
            let mut cursor = node.walk();
            let value = node
                .named_children(&mut cursor)
                .find(|child| child.id() != name_node.id())
                .ok_or_else(|| error(format!("enhanced Query field `{name}` lacks a value")))?;
            Ok(EnhancedQueryExpression::Field {
                name,
                value: Box::new(parse_expression(value, source, predicates)?),
            })
        }
        "negated_field" => {
            let mut cursor = node.walk();
            let name = node
                .named_children(&mut cursor)
                .next()
                .ok_or_else(|| error("enhanced Query negated field lacks a name"))?;
            Ok(EnhancedQueryExpression::NegatedField(node_text(
                name, source,
            )?))
        }
        "list" => {
            let quantifier = query_quantifier(node, source)?;
            let captures = query_captures(node, source)?;
            let alternatives = expression_children(node, source, predicates)?;
            if alternatives.is_empty() {
                return Err(error("enhanced Query alternation is empty"));
            }
            Ok(EnhancedQueryExpression::Alternation {
                quantifier,
                captures,
                alternatives,
            })
        }
        "grouping" => {
            let quantifier = query_quantifier(node, source)?;
            let captures = query_captures(node, source)?;
            let terms = expression_children(node, source, predicates)?;
            if terms.is_empty() {
                return Err(error("enhanced Query group is empty"));
            }
            Ok(EnhancedQueryExpression::Group {
                quantifier,
                captures,
                terms,
            })
        }
        kind => Err(error(format!(
            "enhanced Query structure `{kind}` is not supported by the stable V1 compiler"
        ))),
    }
}

fn expression_children(
    node: tree_sitter::Node<'_>,
    source: &str,
    predicates: &mut Vec<EnhancedQueryPredicate>,
) -> Result<Vec<EnhancedQueryExpression>, EnhancedQueryParseError> {
    let mut expressions = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "(" | ")" | "[" | "]" | "quantifier" | "capture"
        ) {
            continue;
        }
        if child.kind() == "predicate" {
            predicates.push(parse_predicate(child, source)?);
        } else if child.kind() == "." {
            expressions.push(EnhancedQueryExpression::Anchor);
        } else if child.is_named() {
            expressions.push(parse_expression(child, source, predicates)?);
        }
    }
    Ok(expressions)
}

fn parse_predicate(
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Result<EnhancedQueryPredicate, EnhancedQueryParseError> {
    let raw = node_text(node, source)?;
    let head_end = raw
        .find(char::is_whitespace)
        .or_else(|| raw.find(')'))
        .ok_or_else(|| error("enhanced Query predicate head is malformed"))?;
    let head = raw
        .get(2..head_end)
        .ok_or_else(|| error("enhanced Query predicate name is malformed"))?;
    let suffix = raw
        .as_bytes()
        .get(head_end.saturating_sub(1))
        .copied()
        .ok_or_else(|| error("enhanced Query predicate kind is missing"))?;
    let kind = match suffix {
        b'?' => EnhancedQueryPredicateKind::Predicate,
        b'!' => EnhancedQueryPredicateKind::Directive,
        _ => return Err(error("enhanced Query predicate kind is invalid")),
    };
    let name = head.trim_end_matches(['?', '!']).to_owned();
    let mut operands = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "capture" => {
                operands.push(EnhancedQueryOperand::Capture(capture_name(child, source)?));
            }
            "string" => operands.push(EnhancedQueryOperand::String(decode_query_string(
                node_text(child, source)?.as_str(),
            )?)),
            "identifier" => {
                let value = node_text(child, source)?;
                if value != name {
                    operands.push(EnhancedQueryOperand::Bare(value));
                }
            }
            "parameters" => {
                let mut parameter_cursor = child.walk();
                for parameter in child.named_children(&mut parameter_cursor) {
                    match parameter.kind() {
                        "capture" => operands.push(EnhancedQueryOperand::Capture(capture_name(
                            parameter, source,
                        )?)),
                        "string" => operands.push(EnhancedQueryOperand::String(
                            decode_query_string(node_text(parameter, source)?.as_str())?,
                        )),
                        "identifier" => {
                            operands.push(EnhancedQueryOperand::Bare(node_text(parameter, source)?))
                        }
                        other => {
                            return Err(error(format!(
                                "enhanced Query predicate operand `{other}` is unsupported"
                            )));
                        }
                    }
                }
            }
            "predicate_type" => {}
            _ => {}
        }
    }
    Ok(EnhancedQueryPredicate {
        name,
        kind,
        operands,
    })
}

fn query_quantifier(
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Result<EnhancedQueryQuantifier, EnhancedQueryParseError> {
    let Some(quantifier) = node.child_by_field_name("quantifier") else {
        return Ok(EnhancedQueryQuantifier::ONE);
    };
    match node_text(quantifier, source)?.as_str() {
        "?" => Ok(EnhancedQueryQuantifier {
            minimum: 0,
            maximum: Some(1),
        }),
        "+" => Ok(EnhancedQueryQuantifier {
            minimum: 1,
            maximum: None,
        }),
        "*" => Ok(EnhancedQueryQuantifier {
            minimum: 0,
            maximum: None,
        }),
        other => Err(error(format!(
            "enhanced Query quantifier `{other}` is invalid"
        ))),
    }
}

fn query_captures(
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Vec<String>, EnhancedQueryParseError> {
    let mut captures = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "capture" {
            captures.push(capture_name(child, source)?);
        }
    }
    Ok(captures)
}

fn capture_name(
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Result<String, EnhancedQueryParseError> {
    node.child_by_field_name("name")
        .map(|name| node_text(name, source))
        .transpose()?
        .or_else(|| {
            node_text(node, source)
                .ok()
                .map(|raw| raw.trim_start_matches('@').to_owned())
        })
        .filter(|name| !name.is_empty())
        .ok_or_else(|| error("enhanced Query capture name is empty"))
}

fn required_field_text(
    node: tree_sitter::Node<'_>,
    field: &str,
    source: &str,
) -> Result<String, EnhancedQueryParseError> {
    node.child_by_field_name(field)
        .ok_or_else(|| error(format!("enhanced Query {} lacks {field}", node.kind())))
        .and_then(|value| node_text(value, source))
}

fn node_text(
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Result<String, EnhancedQueryParseError> {
    node.utf8_text(source.as_bytes())
        .map(str::to_owned)
        .map_err(|cause| error(format!("read enhanced Query syntax node: {cause}")))
}

fn decode_query_string(raw: &str) -> Result<String, EnhancedQueryParseError> {
    serde_json::from_str(raw)
        .map_err(|cause| error(format!("decode enhanced Query string operand: {cause}")))
}

fn error(message: impl Into<String>) -> EnhancedQueryParseError {
    EnhancedQueryParseError {
        message: message.into(),
    }
}
