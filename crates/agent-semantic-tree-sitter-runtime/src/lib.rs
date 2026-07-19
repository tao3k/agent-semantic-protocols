//! Language-neutral execution for ASP Tree-sitter queries.

mod compiled_query;
mod query_execution;

pub use compiled_query::{CompiledSyntaxQuery, compile_query_source};
pub use query_execution::{
    NativeQueryCapture, NativeQueryExecution, NativeQueryMatch, NativeQueryNode, execute_query,
};
