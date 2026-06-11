//! Shared search-pipe data model.

use std::collections::BTreeMap;

use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Candidate {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) symbol: String,
    pub(super) text: String,
    pub(super) source: String,
    pub(super) confidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SearchPipeSourceTrace {
    pub(super) source: String,
    pub(super) status: String,
    pub(super) matched: usize,
    pub(super) missing: usize,
    pub(super) normalized: usize,
    pub(super) fields: BTreeMap<String, Value>,
}

impl SearchPipeSourceTrace {
    pub(super) fn new(
        source: impl Into<String>,
        status: impl Into<String>,
        matched: usize,
        missing: usize,
        normalized: usize,
    ) -> Self {
        Self {
            source: source.into(),
            status: status.into(),
            matched,
            missing,
            normalized,
            fields: BTreeMap::new(),
        }
    }

    pub(super) fn with_fields(mut self, fields: BTreeMap<String, Value>) -> Self {
        self.fields = fields;
        self
    }

    pub(super) fn compact(&self) -> String {
        let mut compact = format!("{}:{}", self.source, self.status);
        if !self.fields.is_empty() {
            let fields = self
                .fields
                .iter()
                .map(|(key, value)| format!("{key}={}", compact_field_value(value)))
                .collect::<Vec<_>>()
                .join(";");
            compact.push('[');
            compact.push_str(&fields);
            compact.push(']');
        }
        compact
    }
}

fn compact_field_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}
