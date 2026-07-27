use std::path::Path;

#[cfg(unix)]
use std::path::PathBuf;

#[cfg(unix)]
use super::c_family_native::{NativeParseResult, NativeParserLibrary, installed_library_path};

#[cfg(unix)]
#[derive(serde::Deserialize)]
struct CompileCommandEntry {
    directory: PathBuf,
    file: PathBuf,
    arguments: Option<Vec<String>>,
    command: Option<String>,
}

pub(crate) struct NativeProjectionRequest<'a> {
    pub(crate) activation_root: &'a Path,
    pub(crate) project_root: &'a Path,
    pub(crate) owner: &'a Path,
    pub(crate) language_id: &'a str,
    pub(crate) provider_id: &'a str,
    pub(crate) artifact_stem: &'a str,
    pub(crate) abi_version: u32,
    pub(crate) parse_symbol: &'a str,
    pub(crate) free_symbol: &'a str,
}

/// Parse one caller-selected translation unit and shape parser facts for the
/// existing ASP-owned projection import. `None` means the declared
/// external-process fallback must be used.
#[cfg(unix)]
pub(crate) fn try_native_projection(
    request: NativeProjectionRequest<'_>,
) -> Result<Option<Vec<u8>>, String> {
    let library_path = installed_library_path(request.activation_root, request.artifact_stem);
    if !library_path.is_file() {
        return Ok(None);
    }
    let Some(compile_args) = compile_args_for_owner(request.project_root, request.owner)? else {
        return Ok(None);
    };
    let owner = request.owner.to_str().ok_or_else(|| {
        format!(
            "C-family owner path is not UTF-8: {}",
            request.owner.display()
        )
    })?;
    let library = NativeParserLibrary::open(
        &library_path,
        request.abi_version,
        request.parse_symbol,
        request.free_symbol,
    )?;
    let result = library.parse_translation_unit(
        request.project_root,
        owner,
        request.language_id,
        &compile_args,
    )?;
    if !result.errors.is_empty() {
        return Err(format!(
            "native C-family parser failed for {owner}: {}",
            result.errors.join("; ")
        ));
    }
    native_projection_json(request, result).map(Some)
}

#[cfg(not(unix))]
pub(crate) fn try_native_projection(
    request: NativeProjectionRequest<'_>,
) -> Result<Option<Vec<u8>>, String> {
    let NativeProjectionRequest {
        activation_root,
        project_root,
        owner,
        language_id,
        provider_id,
        artifact_stem,
        abi_version,
        parse_symbol,
        free_symbol,
    } = request;
    let _ = (
        activation_root,
        project_root,
        owner,
        language_id,
        provider_id,
        artifact_stem,
        abi_version,
        parse_symbol,
        free_symbol,
    );
    Ok(None)
}

#[cfg(unix)]
fn compile_args_for_owner(
    project_root: &Path,
    owner: &Path,
) -> Result<Option<Vec<String>>, String> {
    let database_path = project_root.join("compile_commands.json");
    let bytes = match std::fs::read(&database_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read {}: {error}",
                database_path.display()
            ));
        }
    };
    let entries: Vec<CompileCommandEntry> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", database_path.display()))?;
    let requested = project_root.join(owner);
    let Some(entry) = entries.into_iter().find(|entry| {
        let candidate = if entry.file.is_absolute() {
            entry.file.clone()
        } else {
            entry.directory.join(&entry.file)
        };
        paths_match(&candidate, &requested)
    }) else {
        return Ok(None);
    };
    let CompileCommandEntry {
        directory,
        file: _,
        arguments,
        command,
    } = entry;
    let arguments = match (arguments, command) {
        (Some(arguments), _) => arguments,
        (None, Some(command)) => split_shell_words(&command)?,
        (None, None) => {
            return Err(format!(
                "compile command for {} has neither arguments nor command",
                owner.display()
            ));
        }
    };
    Ok(Some(normalize_compile_args(
        arguments, &directory, &requested,
    )))
}

#[cfg(unix)]
fn paths_match(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(unix)]
pub(crate) fn normalize_compile_args(
    arguments: Vec<String>,
    directory: &Path,
    requested: &Path,
) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut skip_next = false;
    for (index, argument) in arguments.into_iter().enumerate() {
        if index == 0 || skip_next {
            skip_next = false;
            continue;
        }
        if argument == "-c" {
            continue;
        }
        if argument == "-o" {
            skip_next = true;
            continue;
        }
        if argument.starts_with("-o") && argument.len() > 2 {
            continue;
        }
        let argument_path = Path::new(&argument);
        let absolute_argument = if argument_path.is_absolute() {
            argument_path.to_path_buf()
        } else {
            directory.join(argument_path)
        };
        if paths_match(&absolute_argument, requested) {
            continue;
        }
        normalized.push(argument);
    }
    normalized
}

#[cfg(unix)]
pub(crate) fn split_shell_words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in command.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match (quote, character) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('"'), '\\') | (None, '\\') => escaped = true,
            (None, '\'') | (None, '"') => quote = Some(character),
            (None, character) if character.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if escaped || quote.is_some() {
        return Err("compile command contains an unterminated quote or escape".to_string());
    }
    if !current.is_empty() {
        words.push(current);
    }
    Ok(words)
}

#[cfg(unix)]
pub(crate) fn native_projection_json(
    request: NativeProjectionRequest<'_>,
    result: NativeParseResult,
) -> Result<Vec<u8>, String> {
    let owner = request.owner.to_string_lossy();
    let source_id = format!("source:{owner}");
    let owner_id = format!("owner:{owner}");
    let mut items = std::collections::BTreeMap::new();
    for fact in result.facts {
        if fact.location.path != owner {
            continue;
        }
        let selector = fact.location.structural_selector;
        if selector.is_empty() {
            continue;
        }
        items.entry(selector.clone()).or_insert_with(|| {
            serde_json::json!({
                "itemId": format!("item:{selector}"),
                "ownerId": owner_id,
                "kind": fact.kind,
                "name": fact.name,
                "selector": selector,
            })
        });
    }
    let packet = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-language-projection",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.language-projection",
        "protocolVersion": "1",
        "languageId": request.language_id,
        "harness": {
            "harnessId": request.provider_id,
            "parserAbi": format!("ccls-asp-native-abi-{}", request.abi_version),
            "selectorDialect": "c-family-v1",
        },
        "sources": [{
            "sourceId": source_id,
            "path": owner,
            "sourceKind": "source",
        }],
        "owners": [{
            "ownerId": owner_id,
            "sourceId": source_id,
            "kind": "translation-unit",
            "name": owner,
        }],
        "items": items.into_values().collect::<Vec<_>>(),
        "relations": [],
    });
    serde_json::to_vec(&packet)
        .map_err(|error| format!("failed to encode native C-family projection: {error}"))
}
