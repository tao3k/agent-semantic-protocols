//! Public bounded structured-filter facade.

pub use crate::structured_projection::{
    BoundedPathCommandSpec, BoundedPathSegment, StructuredFilterClassification,
    classify_bounded_path_filter, classify_single_bounded_path_command,
    classify_single_bounded_path_tokens,
};
