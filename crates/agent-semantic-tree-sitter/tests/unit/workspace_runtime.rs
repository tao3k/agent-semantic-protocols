use crate::{
    NativeQueryExecution, compile_native_query_source, execute_native_query,
    registered_language_grammar,
};

fn execute_rust_query(query_source: &str, source: &str) -> NativeQueryExecution {
    let language = registered_language_grammar("rust").expect("Rust grammar is registered by ASP");
    let query = compile_native_query_source(&language, query_source)
        .expect("tree-sitter query compiles through the canonical runtime");

    execute_native_query(&language, &query, source)
        .expect("tree-sitter query executes through the canonical runtime")
}

fn capture_count(execution: &NativeQueryExecution) -> usize {
    execution
        .matches
        .iter()
        .map(|query_match| query_match.captures.len())
        .sum()
}

#[test]
fn registered_rust_grammar_executes_string_predicates() {
    let execution = execute_rust_query(
        r#"((string_literal) @value (#match? @value "asp install plugin --codex"))"#,
        r#"
fn install_examples() {
    let matching = "asp install plugin --codex";
    let ignored = "asp install plugin --claude";
}
"#,
    );

    assert!(execution.parsed);
    assert_eq!(capture_count(&execution), 1);
    assert_eq!(execution.matches[0].captures[0].capture_name, "value");
}

#[test]
fn registered_rust_grammar_executes_multi_node_queries() {
    let execution = execute_rust_query(
        r#"
[
  (function_item name: (identifier) @declaration)
  (struct_item name: (type_identifier) @declaration)
  (enum_item name: (type_identifier) @declaration)
  (trait_item name: (type_identifier) @declaration)
  (type_item name: (type_identifier) @declaration)
]
"#,
        r#"
fn run() {}
struct Record;
enum State { Ready }
trait Execute {}
type Alias = usize;
"#,
    );

    assert!(execution.parsed);
    assert_eq!(capture_count(&execution), 5);
    assert!(execution.matches.iter().all(|query_match| {
        query_match
            .captures
            .iter()
            .all(|capture| capture.capture_name == "declaration")
    }));
}

#[test]
fn unregistered_languages_fail_closed() {
    let error = registered_language_grammar("not-registered")
        .expect_err("unknown languages must not fall back to a guessed grammar");

    assert!(error.contains("not-registered"));
}
