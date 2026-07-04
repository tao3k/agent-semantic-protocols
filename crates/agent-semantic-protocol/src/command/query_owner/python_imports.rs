use std::fs;
use std::path::{Path, PathBuf};

use super::item::OwnerItem;
use super::tree_sitter_items::collect_tree_sitter_owner_items;

pub(super) struct ImportedOwnerItems {
    pub(super) path: PathBuf,
    pub(super) source: String,
    pub(super) items: Vec<OwnerItem>,
}

pub(super) fn python_imported_owner_items(
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
    let items = collect_tree_sitter_owner_items("python", &target_source, &target_path)?
        .unwrap_or_default();
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
    let mut bindings = Vec::new();
    for line in source.lines() {
        let line = strip_python_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("from ") {
            collect_python_from_import_bindings(rest, &mut bindings);
        } else if let Some(rest) = line.strip_prefix("import ") {
            collect_python_import_bindings(rest, &mut bindings);
        }
    }
    Ok(bindings)
}

fn strip_python_comment(line: &str) -> &str {
    line.split_once('#')
        .map_or(line, |(before_comment, _)| before_comment)
}

fn collect_python_from_import_bindings(rest: &str, bindings: &mut Vec<PythonImportBinding>) {
    let Some((module, names)) = rest.split_once(" import ") else {
        return;
    };
    let module = module.trim();
    if module.is_empty() {
        return;
    }
    for name in split_python_import_names(names) {
        let Some((name, alias)) = parse_python_import_alias(&name) else {
            continue;
        };
        if name == "*" {
            continue;
        }
        bindings.push(PythonImportBinding {
            module: Some(module.to_string()),
            name,
            alias,
        });
    }
}

fn collect_python_import_bindings(rest: &str, bindings: &mut Vec<PythonImportBinding>) {
    for name in split_python_import_names(rest) {
        let Some((name, alias)) = parse_python_import_alias(&name) else {
            continue;
        };
        bindings.push(PythonImportBinding {
            module: Some(name.clone()),
            name,
            alias,
        });
    }
}

fn split_python_import_names(names: &str) -> Vec<String> {
    names
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.trim_end_matches(';').trim().to_string())
        .collect()
}

fn parse_python_import_alias(name: &str) -> Option<(String, Option<String>)> {
    let mut parts = name.split_whitespace();
    let imported = parts.next()?.to_string();
    match (parts.next(), parts.next(), parts.next()) {
        (None, None, None) => Some((imported, None)),
        (Some("as"), Some(alias), None) => Some((imported, Some(alias.to_string()))),
        _ => None,
    }
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
