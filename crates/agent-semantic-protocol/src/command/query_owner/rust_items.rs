use std::path::Path;

use syn::spanned::Spanned;

use super::item::OwnerItem;

pub(in crate::command) fn collect_syn_rust_owner_items(
    source: &str,
    path: &Path,
) -> Result<Vec<OwnerItem>, String> {
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
            if let Some(name) = rust_impl_owner_name(item) {
                push_item(items, name, "impl", item);
            }
            for impl_item in &item.items {
                if let syn::ImplItem::Fn(function) = impl_item {
                    push_item(items, function.sig.ident.to_string(), "method", function);
                }
            }
        }
        _ => {}
    }
}

fn rust_impl_owner_name(item: &syn::ItemImpl) -> Option<String> {
    rust_type_owner_name(&item.self_ty)
}

fn rust_type_owner_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Array(array) => rust_type_owner_name(&array.elem),
        syn::Type::Group(group) => rust_type_owner_name(&group.elem),
        syn::Type::Paren(paren) => rust_type_owner_name(&paren.elem),
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Ptr(ptr) => rust_type_owner_name(&ptr.elem),
        syn::Type::Reference(reference) => rust_type_owner_name(&reference.elem),
        syn::Type::Slice(slice) => rust_type_owner_name(&slice.elem),
        syn::Type::Tuple(tuple) if tuple.elems.len() == 1 => {
            tuple.elems.iter().next().and_then(rust_type_owner_name)
        }
        _ => None,
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

fn rust_syntax_node_for_kind(kind: &str) -> &'static str {
    match kind {
        "const" => "const_item",
        "enum" => "enum_item",
        "function" => "function_item",
        "impl" => "impl_item",
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
