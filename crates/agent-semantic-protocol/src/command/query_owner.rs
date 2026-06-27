//! ASP-owned bounded owner queries.
//!
//! This path handles explicit owner-file queries without spawning a language
//! provider. It is intentionally narrow: native providers remain the authority
//! for project-wide semantic enrichment, while ASP owns the cheap locator/code
//! contract for a known owner file.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use agent_semantic_tree_sitter::{
    BuiltinCatalogId, BuiltinCatalogLanguageId, builtin_catalog_source,
};
use syn::spanned::Spanned;
use tree_sitter::StreamingIterator;

use super::query_owner_item::{
    OwnerItem, collect_gerbil_query_owner_items, owner_item_matches_request,
};
use super::query_owner_structural_selector::parse_structural_owner_query;

pub(super) fn run_asp_fast_owner_query_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    locator_root: &Path,
) -> Result<bool, String> {
    let Some(request) = OwnerQueryRequest::parse(language_id, args) else {
        return Ok(false);
    };
    let Some(path) = resolve_owner_path(project_root, locator_root, &request.owner_path) else {
        if owner_path_is_file_like(&request.owner_path) {
            render_unresolved_owner_query(&request)?;
            return Ok(true);
        }
        return Ok(false);
    };
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let items = if language_id == "rust" {
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            render_non_source_owner_query(&request, &path, project_root, locator_root, &source)?;
            return Ok(true);
        }
        collect_syn_rust_owner_items(&source, &path)?
    } else if language_id == "gerbil-scheme" {
        collect_gerbil_query_owner_items(&source)
    } else {
        let Some(items) = collect_tree_sitter_owner_items(language_id, &source, &path)? else {
            render_non_source_owner_query(&request, &path, project_root, locator_root, &source)?;
            return Ok(true);
        };
        items
    };
    let matches = items
        .iter()
        .filter(|item| {
            owner_item_matches_request(
                item,
                &request.language_id,
                &request.term,
                request.kind.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    if language_id == "python" && matches.is_empty() {
        if let Some(imported) =
            python_imported_owner_items(project_root, locator_root, &path, &source, &request.term)?
        {
            let imported_matches = imported
                .items
                .iter()
                .filter(|item| {
                    owner_item_matches_request(
                        item,
                        &request.language_id,
                        &request.term,
                        request.kind.as_deref(),
                    )
                })
                .collect::<Vec<_>>();
            if !imported_matches.is_empty() {
                if request.code {
                    render_code_matches(&imported.source, &imported_matches)?;
                } else {
                    render_locator_matches(
                        &request,
                        &imported.path,
                        project_root,
                        locator_root,
                        imported.source.lines().count(),
                        &imported_matches,
                    )?;
                }
                return Ok(true);
            }
        }
        if request.code {
            render_code_matches(&source, &[])?;
            return Ok(true);
        }
        return Ok(false);
    }

    if request.code {
        render_code_matches(&source, &matches)?;
    } else {
        render_locator_matches(
            &request,
            &path,
            project_root,
            locator_root,
            source.lines().count(),
            &matches,
        )?;
    }
    Ok(true)
}

struct OwnerQueryRequest {
    language_id: String,
    owner_path: PathBuf,
    kind: Option<String>,
    term: String,
    names_only: bool,
    code: bool,
    projection: &'static str,
}

impl OwnerQueryRequest {
    fn parse(language_id: &str, args: &[String]) -> Option<Self> {
        if !matches!(
            language_id,
            "rust" | "typescript" | "python" | "julia" | "gerbil-scheme"
        ) || !matches!(args.first().map(String::as_str), Some("query"))
        {
            return None;
        }
        if let Some(request) = Self::parse_structural_selector(language_id, args) {
            return Some(request);
        }
        if has_any_arg(
            args,
            &[
                "--json",
                "--receipt-json",
                "--treesitter-query",
                "--catalog",
                "--from-hook",
                "--selector",
            ],
        ) {
            return None;
        }
        let term = arg_value(args, "--term")
            .or_else(|| arg_value(args, "--query"))
            .map(ToString::to_string)?;
        let owner_path = first_positional_owner_arg(args)?;
        if owner_path == "." || owner_path.contains(':') {
            return None;
        }
        Some(Self {
            language_id: language_id.to_string(),
            owner_path: PathBuf::from(owner_path),
            kind: None,
            term,
            names_only: args.iter().any(|arg| arg == "--names-only"),
            code: args.iter().any(|arg| arg == "--code"),
            projection: "outline",
        })
    }

    fn parse_structural_selector(language_id: &str, args: &[String]) -> Option<Self> {
        let selector = arg_value(args, "--selector")?;
        let from_hook = arg_value(args, "--from-hook").unwrap_or_else(|| {
            if args.iter().any(|arg| arg == "--code") {
                "query-code"
            } else {
                "syntax-outline"
            }
        });
        let structural = parse_structural_owner_query(language_id, from_hook, selector)?;
        Some(Self {
            language_id: language_id.to_string(),
            owner_path: structural.owner_path,
            kind: structural.kind,
            term: structural.term,
            names_only: args.iter().any(|arg| arg == "--names-only"),
            code: args.iter().any(|arg| arg == "--code"),
            projection: structural.projection,
        })
    }
}

struct ImportedOwnerItems {
    path: PathBuf,
    source: String,
    items: Vec<OwnerItem>,
}

fn collect_syn_rust_owner_items(source: &str, path: &Path) -> Result<Vec<OwnerItem>, String> {
    let parsed = syn::parse_file(source)
        .map_err(|error| format!("failed to parse Rust owner {}: {error}", path.display()))?;
    Ok(collect_rust_owner_items(&parsed))
}

fn collect_rust_owner_items(file: &syn::File) -> Vec<OwnerItem> {
    let mut items = Vec::new();
    for item in &file.items {
        collect_rust_item(item, &mut items);
    }
    items
}

fn collect_rust_item(item: &syn::Item, items: &mut Vec<OwnerItem>) {
    match item {
        syn::Item::Const(item) => push_item(items, item.ident.to_string(), "const", item),
        syn::Item::Enum(item) => push_item(items, item.ident.to_string(), "enum", item),
        syn::Item::Fn(item) => push_item(items, item.sig.ident.to_string(), "function", item),
        syn::Item::Macro(item) => {
            if let Some(ident) = item.mac.path.segments.last().map(|segment| &segment.ident) {
                push_item(items, ident.to_string(), "macro", item);
            }
        }
        syn::Item::Mod(item) => {
            push_item(items, item.ident.to_string(), "module", item);
            if let Some((_, nested_items)) = &item.content {
                for nested in nested_items {
                    collect_rust_item(nested, items);
                }
            }
        }
        syn::Item::Static(item) => push_item(items, item.ident.to_string(), "static", item),
        syn::Item::Struct(item) => push_item(items, item.ident.to_string(), "struct", item),
        syn::Item::Trait(item) => {
            push_item(items, item.ident.to_string(), "trait", item);
            for trait_item in &item.items {
                if let syn::TraitItem::Fn(function) = trait_item {
                    push_item(
                        items,
                        function.sig.ident.to_string(),
                        "trait-function",
                        function,
                    );
                }
            }
        }
        syn::Item::Type(item) => push_item(items, item.ident.to_string(), "type", item),
        syn::Item::Union(item) => push_item(items, item.ident.to_string(), "union", item),
        syn::Item::Impl(item) => {
            for impl_item in &item.items {
                if let syn::ImplItem::Fn(function) = impl_item {
                    push_item(items, function.sig.ident.to_string(), "method", function);
                }
            }
        }
        _ => {}
    }
}

fn push_item<T: Spanned>(items: &mut Vec<OwnerItem>, name: String, kind: &'static str, node: &T) {
    let span = node.span();
    let start_line = span.start().line.max(1);
    let end_line = span.end().line.max(start_line);
    items.push(OwnerItem {
        name,
        kind,
        syntax_node: rust_syntax_node_for_kind(kind),
        start_line,
        end_line,
    });
}

fn collect_tree_sitter_owner_items(
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

fn python_imported_owner_items(
    project_root: &Path,
    locator_root: &Path,
    owner_path: &Path,
    source: &str,
    term: &str,
) -> Result<Option<ImportedOwnerItems>, String> {
    let Some(target_path) =
        python_import_target(project_root, locator_root, owner_path, source, term)?
    else {
        return Ok(None);
    };
    let target_source = fs::read_to_string(&target_path)
        .map_err(|error| format!("failed to read {}: {error}", target_path.display()))?;
    let Some(items) = collect_tree_sitter_owner_items("python", &target_source, &target_path)?
    else {
        return Ok(None);
    };
    Ok(Some(ImportedOwnerItems {
        path: target_path,
        source: target_source,
        items,
    }))
}

fn python_import_target(
    project_root: &Path,
    locator_root: &Path,
    owner_path: &Path,
    source: &str,
    term: &str,
) -> Result<Option<PathBuf>, String> {
    for binding in python_import_bindings(source)? {
        if binding.bound_name() != term {
            continue;
        }
        if let Some(path) =
            resolve_python_import_path(project_root, locator_root, owner_path, &binding)
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

struct PythonImportBinding {
    module: Option<String>,
    name: String,
    alias: Option<String>,
}

impl PythonImportBinding {
    fn bound_name(&self) -> &str {
        self.alias
            .as_deref()
            .unwrap_or_else(|| self.name.rsplit('.').next().unwrap_or(&self.name))
    }
}

fn python_import_bindings(source: &str) -> Result<Vec<PythonImportBinding>, String> {
    let language: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let catalog = builtin_catalog_source(
        BuiltinCatalogLanguageId::from("python"),
        BuiltinCatalogId::from("imports"),
    )
    .ok_or_else(|| "missing built-in python imports catalog".to_string())?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|error| format!("failed to set python tree-sitter language: {error}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "failed to parse python imports with tree-sitter".to_string())?;
    let query = tree_sitter::Query::new(&language, catalog)
        .map_err(|error| format!("failed to compile python imports query: {error:?}"))?;
    let capture_names = query.capture_names();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut query_matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    let mut bindings = Vec::new();
    while let Some(query_match) = query_matches.next() {
        if let Some(binding) = python_import_binding_from_captures(
            source.as_bytes(),
            capture_names,
            query_match.captures,
        ) {
            bindings.push(binding);
        }
    }
    collect_python_import_from_nodes(tree.root_node(), source.as_bytes(), &mut bindings);
    Ok(bindings)
}

fn collect_python_import_from_nodes(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    bindings: &mut Vec<PythonImportBinding>,
) {
    if node.kind() == "import_from_statement" {
        if let Some(text) = node_text(node, source) {
            bindings.extend(parse_python_import_from_statement(&text));
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_python_import_from_nodes(child, source, bindings);
    }
}

fn parse_python_import_from_statement(text: &str) -> Vec<PythonImportBinding> {
    let Some(rest) = text.trim().strip_prefix("from ") else {
        return Vec::new();
    };
    let Some((module, names)) = rest.split_once(" import ") else {
        return Vec::new();
    };
    names
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let (name, alias) = part
                .split_once(" as ")
                .map_or((part, None), |(name, alias)| {
                    (name.trim(), Some(alias.trim()))
                });
            Some(PythonImportBinding {
                module: Some(module.trim().to_string()),
                name: name.to_string(),
                alias: alias.map(ToString::to_string),
            })
        })
        .collect()
}

fn python_import_binding_from_captures(
    source: &[u8],
    capture_names: &[&str],
    captures: &[tree_sitter::QueryCapture<'_>],
) -> Option<PythonImportBinding> {
    let mut module = None;
    let mut name = None;
    let mut alias = None;
    for capture in captures {
        let capture_name = capture_names.get(capture.index as usize).copied()?;
        match capture_name {
            "import.path" => module = node_text(capture.node, source),
            "import.name" => name = node_text(capture.node, source),
            "import.alias" => alias = node_text(capture.node, source),
            _ => {}
        }
    }
    name.map(|name| PythonImportBinding {
        module,
        name,
        alias,
    })
}

fn resolve_python_import_path(
    project_root: &Path,
    locator_root: &Path,
    owner_path: &Path,
    binding: &PythonImportBinding,
) -> Option<PathBuf> {
    let module = binding.module.as_ref()?;
    let module = module.trim_start_matches('.');
    if module.is_empty() {
        return None;
    }
    let relative = PathBuf::from(format!("{}.py", module.replace('.', "/")));
    let owner_dir = owner_path.parent().unwrap_or(owner_path);
    [
        owner_dir.join(&relative),
        locator_root.join(&relative),
        project_root.join(&relative),
    ]
    .into_iter()
    .find(|path| path.is_file())
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
    match language_id {
        "typescript" => match path.extension().and_then(|extension| extension.to_str()) {
            Some("ts") => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Some("tsx") => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            _ => None,
        },
        "python" => (path.extension().and_then(|extension| extension.to_str()) == Some("py"))
            .then(|| tree_sitter_python::LANGUAGE.into()),
        "julia" => (path.extension().and_then(|extension| extension.to_str()) == Some("jl"))
            .then(|| tree_sitter_julia::LANGUAGE.into()),
        _ => None,
    }
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

fn render_code_matches(source: &str, matches: &[&OwnerItem]) -> Result<(), String> {
    let mut stdout = io::stdout();
    for (index, item) in matches.iter().enumerate() {
        if index > 0 {
            stdout
                .write_all(b"\n")
                .map_err(|error| format!("failed to write owner query stdout: {error}"))?;
        }
        stdout
            .write_all(select_line_range(source, item.start_line, item.end_line).as_bytes())
            .map_err(|error| format!("failed to write owner query stdout: {error}"))?;
    }
    Ok(())
}

fn render_locator_matches(
    request: &OwnerQueryRequest,
    path: &Path,
    project_root: &Path,
    locator_root: &Path,
    line_count: usize,
    matches: &[&OwnerItem],
) -> Result<(), String> {
    let output = if request.names_only {
        "names"
    } else {
        "locator"
    };
    let display_path = display_relative_path(path, project_root, locator_root);
    let mut rendered = String::new();
    rendered.push_str(&format!(
        "[search-owner] q={} pkg=. own=1 item={} itemQuery={} output={output}\n",
        display_path.display(),
        matches.len(),
        request.term
    ));
    rendered.push_str(&format!(
        "|owner {} role=source source=asp-syn-owner lines={line_count}\n",
        display_path.display()
    ));
    for item in matches {
        let structural_selector = format!(
            "{}://{}#item/{}/{}",
            request.language_id,
            display_path.display(),
            item.kind.replace(char::is_whitespace, "-"),
            item.name.replace(char::is_whitespace, "-")
        );
        rendered.push_str(&format!(
            "|item name={} kind={} owner={} structuralSelector={} displayLineRange={}:{} sourceLocatorHint={}:{}:{} syn=node:{} projection={} codePolicy=code-after-exact-selector\n",
            item.name,
            item.kind,
            display_path.display(),
            structural_selector,
            item.start_line,
            item.end_line,
            display_path.display(),
            item.start_line,
            item.end_line,
            item.syntax_node,
            request.projection
        ));
    }
    if matches.is_empty() {
        rendered.push_str(&format!(
            "|query itemQuery={} status=miss match=none item=0 reason=asp-syn-owner-query output={output} next=revise-query\n",
            request.term
        ));
    } else {
        rendered.push_str(&format!(
            "|query itemQuery={} status=hit match=exact item={} reason=asp-syn-owner-query output={output} next=query --code codePolicy=requires-exact-code\n",
            request.term,
            matches.len()
        ));
    }
    io::stdout()
        .write_all(rendered.as_bytes())
        .map_err(|error| format!("failed to write owner query stdout: {error}"))
}

fn render_non_source_owner_query(
    request: &OwnerQueryRequest,
    path: &Path,
    project_root: &Path,
    locator_root: &Path,
    source: &str,
) -> Result<(), String> {
    if request.code {
        render_code_matches(source, &[])
    } else {
        render_locator_matches(
            request,
            path,
            project_root,
            locator_root,
            source.lines().count(),
            &[],
        )
    }
}

fn render_unresolved_owner_query(request: &OwnerQueryRequest) -> Result<(), String> {
    if request.code {
        return Ok(());
    }
    let output = if request.names_only {
        "names"
    } else {
        "locator"
    };
    let display_path = request.owner_path.to_string_lossy().replace('\\', "/");
    let rendered = format!(
        "[search-owner] q={display_path} pkg=. own=0 item=0 itemQuery={} output={output}\n|query itemQuery={} status=miss match=none item=0 reason=owner-not-found output={output} next=search-owner\n",
        request.term, request.term
    );
    io::stdout()
        .write_all(rendered.as_bytes())
        .map_err(|error| format!("failed to write owner query stdout: {error}"))
}

fn owner_path_is_file_like(path: &Path) -> bool {
    path.extension().is_some() || path.components().count() > 1
}

fn select_line_range(source: &str, start: usize, end: usize) -> String {
    source
        .split_inclusive('\n')
        .skip(start.saturating_sub(1))
        .take(end.saturating_sub(start).saturating_add(1))
        .collect()
}

fn display_relative_path<'a>(
    path: &'a Path,
    project_root: &'a Path,
    locator_root: &'a Path,
) -> &'a Path {
    path.strip_prefix(locator_root)
        .or_else(|_| path.strip_prefix(project_root))
        .unwrap_or(path)
}

fn rust_syntax_node_for_kind(kind: &str) -> &'static str {
    match kind {
        "const" => "const_item",
        "enum" => "enum_item",
        "function" => "function_item",
        "macro" => "macro_invocation",
        "method" => "function_item",
        "module" => "mod_item",
        "static" => "static_item",
        "struct" => "struct_item",
        "trait" => "trait_item",
        "trait-function" => "function_signature_item",
        "type" => "type_item",
        "union" => "union_item",
        _ => "item",
    }
}

fn resolve_owner_path(
    project_root: &Path,
    locator_root: &Path,
    owner_path: &Path,
) -> Option<PathBuf> {
    let candidates = if owner_path.is_absolute() {
        vec![owner_path.to_path_buf()]
    } else {
        vec![locator_root.join(owner_path), project_root.join(owner_path)]
    };
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn first_positional_owner_arg(args: &[String]) -> Option<&str> {
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if arg.starts_with("--") {
            index += if option_takes_value(arg) { 2 } else { 1 };
            continue;
        }
        if arg.starts_with('-') || arg == "." {
            index += 1;
            continue;
        }
        return Some(arg);
    }
    None
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let prefix = format!("{flag}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix))
        .or_else(|| {
            args.windows(2)
                .find_map(|window| (window[0] == flag).then_some(window[1].as_str()))
        })
}

fn has_any_arg(args: &[String], flags: &[&str]) -> bool {
    args.iter().any(|arg| {
        flags
            .iter()
            .any(|flag| arg == flag || arg.starts_with(&format!("{flag}=")))
    })
}

fn option_takes_value(arg: &str) -> bool {
    if arg.contains('=') {
        return false;
    }
    matches!(
        arg,
        "--term"
            | "--query"
            | "--workspace"
            | "--source"
            | "--from-hook"
            | "--selector"
            | "--treesitter-query"
            | "--catalog"
            | "--view"
    )
}
