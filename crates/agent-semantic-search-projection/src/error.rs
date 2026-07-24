use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchProjectionError {
    InvalidPacket(String),
    InvalidRequest(String),
    BudgetExceeded {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

impl Display for SearchProjectionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPacket(message) => write!(formatter, "invalid search packet: {message}"),
            Self::InvalidRequest(message) => {
                write!(formatter, "invalid search projection request: {message}")
            }
            Self::BudgetExceeded {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "search projection exceeds byte budget: actual={actual_bytes} max={max_bytes}"
            ),
        }
    }
}

impl Error for SearchProjectionError {}
