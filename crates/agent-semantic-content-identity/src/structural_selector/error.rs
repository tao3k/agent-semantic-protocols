//! Defines fail-closed structural-selector codec errors.

use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::{self};

#[derive(Clone, Debug, Eq, PartialEq)]
/// Offset-bearing failure produced by structural-selector decoding.
pub struct StructuralSelectorCodecError {
    message: String,
}

impl StructuralSelectorCodecError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for StructuralSelectorCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for StructuralSelectorCodecError {}
