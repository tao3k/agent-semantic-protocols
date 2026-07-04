use std::path::Path;

use agent_semantic_tree_sitter::{
    BuiltinCatalogId, BuiltinCatalogLanguageId, builtin_catalog_source,
};
use tree_sitter::StreamingIterator;

use super::item::OwnerItem;

pub(super) fn collect_tree_sitter_owner_items(
    language_id: &str,
    source: &str,
    path: &Path,
) -> Result<Option<Vec<OwnerItem>>, String> {
    let Some(language) = tree_sitter_language(language_id, path) else {
        return Ok(None);
    };
    let Some(catalog) = builtin_catalog_source(
        BuiltinCatalogLanguageId::from(language_id),
        BuiltinCatalogId::from("declarations"),
    ) else {
        return Ok(None);
    };

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|error| format!("failed to set {language_id} tree-sitter language: {error}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| format!("failed to parse {} with tree-sitter", path.display()))?;
    if tree.root_node().has_error() {
        return Err(format!(
            "tree-sitter parse reported errors for {}",
            path.display()
        ));
    }

    let query = tree_sitter::Query::new(&language, catalog).map_err(|error| {
        format!("failed to compile {language_id} declarations query: {error:?}")
    })?;
    let capture_names = query.capture_names();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut query_matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    let mut items = Vec::new();
    while let Some(query_match) = query_matches.next() {
        if let Some(item) = owner_item_from_query_captures(
            language_id,
            source.as_bytes(),
            capture_names,
            query_match.captures,
        ) {
            items.push(item);
        }
    }
    Ok(Some(items))
}

fn owner_item_from_query_captures<'tree>(
    language_id: &str,
    source: &[u8],
    capture_names: &[&str],
    captures: &[tree_sitter::QueryCapture<'tree>],
) -> Option<OwnerItem> {
    let (capture_name, name_node, definition_node) = query_capture_nodes(capture_names, captures)?;
    let name = node_text(name_node, source)?;
    let kind = capture_name
        .split_once('.')
        .map(|(kind, _)| kind)
        .unwrap_or("item");
    let definition_node = definition_node
        .or_else(|| enclosing_definition_node(language_id, kind, name_node))
        .unwrap_or(name_node);
    let start = definition_node.start_position().row.saturating_add(1);
    let end = definition_node
        .end_position()
        .row
        .saturating_add(1)
        .max(start);
    Some(OwnerItem {
        name,
        kind: stable_kind(kind),
        syntax_node: definition_node.kind(),
        start_line: start,
        end_line: end,
    })
}

fn query_capture_nodes<'names, 'tree>(
    capture_names: &'names [&'names str],
    captures: &[tree_sitter::QueryCapture<'tree>],
) -> Option<(
    &'names str,
    tree_sitter::Node<'tree>,
    Option<tree_sitter::Node<'tree>>,
)> {
    let mut name_capture = None::<(&str, tree_sitter::Node<'tree>)>;
    let mut definition_node = None::<tree_sitter::Node<'tree>>;
    for capture in captures {
        let Some(capture_name) = capture_names.get(capture.index as usize).copied() else {
            continue;
        };
        if capture_name.ends_with(".name") {
            name_capture = Some((capture_name, capture.node));
        } else if capture_name.ends_with(".definition") {
            definition_node = Some(capture.node);
        }
    }
    name_capture.map(|(capture_name, name_node)| (capture_name, name_node, definition_node))
}

fn enclosing_definition_node<'tree>(
    language_id: &str,
    capture_kind: &str,
    node: tree_sitter::Node<'tree>,
) -> Option<tree_sitter::Node<'tree>> {
    let mut current = Some(node);
    while let Some(node) = current {
        if is_definition_node(language_id, capture_kind, node.kind()) {
            return Some(node);
        }
        current = node.parent();
    }
    None
}

fn is_definition_node(language_id: &str, capture_kind: &str, node_kind: &str) -> bool {
    match (language_id, capture_kind) {
        ("julia", "constant") => node_kind == "assignment",
        ("julia", "function") => node_kind == "function_definition",
        ("julia", "macro") => node_kind == "macro_definition",
        ("julia", "module") => node_kind == "module_definition",
        ("julia", "type") => matches!(
            node_kind,
            "abstract_definition" | "primitive_definition" | "struct_definition"
        ),
        ("python", "class") => node_kind == "class_definition",
        ("python", "function") => node_kind == "function_definition",
        ("typescript", "class") => node_kind == "class_declaration",
        ("typescript", "enum") => node_kind == "enum_declaration",
        ("typescript", "function") => node_kind == "function_declaration",
        ("typescript", "interface") => node_kind == "interface_declaration",
        ("typescript", "type") => node_kind == "type_alias_declaration",
        ("typescript", "variable") => {
            matches!(node_kind, "lexical_declaration" | "variable_declarator")
        }
        _ => false,
    }
}

fn tree_sitter_language(language_id: &str, path: &Path) -> Option<tree_sitter::Language> {
    let _ = (language_id, path);
    None
}

fn node_text(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    node.utf8_text(source).ok().map(ToString::to_string)
}

fn stable_kind(kind: &str) -> &'static str {
    match kind {
        "class" => "class",
        "constant" => "constant",
        "enum" => "enum",
        "function" => "function",
        "interface" => "interface",
        "macro" => "macro",
        "module" => "module",
        "type" => "type",
        "variable" => "variable",
        _ => "item",
    }
}
