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

pub(crate) struct NativeProjectionOutput {
    pub(crate) language_projection: Vec<u8>,
    pub(crate) structural_index_packet: Vec<u8>,
    pub(crate) generation: agent_semantic_client_core::ClientCacheGeneration,
    pub(crate) source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
}

#[cfg(unix)]
pub(crate) fn native_library_available(activation_root: &Path, artifact_stem: &str) -> bool {
    installed_library_path(activation_root, artifact_stem).is_file()
}

#[cfg(not(unix))]
pub(crate) fn native_library_available(_activation_root: &Path, _artifact_stem: &str) -> bool {
    false
}

/// Parse one caller-selected translation unit and shape parser facts for the
/// ASP-owned projection/import path. Callers with a declared native capability
/// must treat `None` as a hard availability error, never as permission to spawn
/// the legacy provider process.
#[cfg(unix)]
pub(crate) fn try_native_projection(
    request: NativeProjectionRequest<'_>,
    source_snapshot: &agent_semantic_client::source_index::CurrentSourceIndexSnapshot,
) -> Result<Option<NativeProjectionOutput>, String> {
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
    let language_projection = native_projection_json(&request, &result)?;
    let (generation, structural_index_packet) =
        native_structural_index_packet(&request, &result, source_snapshot)?;
    Ok(Some(NativeProjectionOutput {
        language_projection,
        structural_index_packet,
        generation,
        source_snapshot: source_snapshot.source_snapshot.clone(),
    }))
}

#[cfg(not(unix))]
pub(crate) fn try_native_projection(
    request: NativeProjectionRequest<'_>,
    source_snapshot: &agent_semantic_client::source_index::CurrentSourceIndexSnapshot,
) -> Result<Option<NativeProjectionOutput>, String> {
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
        source_snapshot,
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
    request: &NativeProjectionRequest<'_>,
    result: &NativeParseResult,
) -> Result<Vec<u8>, String> {
    let owner = request.owner.to_string_lossy();
    let source_id = format!("source:{owner}");
    let owner_id = format!("owner:{owner}");
    let mut items = std::collections::BTreeMap::new();
    for fact in &result.facts {
        if fact.location.path != owner {
            continue;
        }
        let selector = &fact.location.structural_selector;
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

#[cfg(unix)]
fn native_structural_index_packet(
    request: &NativeProjectionRequest<'_>,
    result: &NativeParseResult,
    snapshot: &agent_semantic_client::source_index::CurrentSourceIndexSnapshot,
) -> Result<(agent_semantic_client_core::ClientCacheGeneration, Vec<u8>), String> {
    use sha2::{Digest, Sha256};

    let requested_owner = request.owner.to_string_lossy().into_owned();
    let mut owner_paths = std::collections::BTreeSet::from([requested_owner.clone()]);
    owner_paths.extend(result.facts.iter().map(|fact| fact.location.path.clone()));
    owner_paths.extend(
        result
            .dependency_usages
            .iter()
            .map(|dependency| dependency.owner_path.clone()),
    );
    owner_paths.retain(|owner| snapshot.source_blobs.get(&owner.as_str().into()).is_some());
    if !owner_paths.contains(&requested_owner) {
        return Err(format!(
            "native C-family source snapshot omitted requested owner: {requested_owner}"
        ));
    }

    let file_hashes = owner_paths
        .iter()
        .map(|owner| {
            let bytes = snapshot
                .source_blobs
                .get(&owner.as_str().into())
                .ok_or_else(|| format!("source snapshot omitted C-family owner: {owner}"))?;
            Ok(agent_semantic_client_core::ClientCacheFileHash {
                path: owner.clone(),
                sha256: format!("{:x}", Sha256::digest(bytes)),
                byte_len: bytes.len() as u64,
                mtime_ms: 0,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let generation_id = agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
        &snapshot.source_snapshot,
    );
    let generation = agent_semantic_client_core::ClientCacheGeneration {
        generation_id: generation_id.clone(),
        language_id: request.language_id.into(),
        provider_id: request.provider_id.into(),
        provider_version: None,
        export_method: Some("native-library".to_string()),
        project_root: request.project_root.display().to_string(),
        package_root: None,
        schema_ids: vec!["agent.semantic-protocols.semantic-structural-index".into()],
        cache_status: agent_semantic_client_core::CacheStatus::WarmProvider,
        raw_source_stored: false,
        request_fingerprint: result
            .compile_contexts
            .first()
            .map(|context| context.digest.clone()),
        file_hashes: Some(file_hashes.clone()),
        artifact_ids: None,
    };

    let owners = owner_paths
        .iter()
        .map(|owner| {
            serde_json::json!({
                "ownerPath": owner,
                "ownerKind": if owner == &requested_owner {
                    "translation-unit"
                } else {
                    "included-source"
                },
                "sourceAuthority": "ccls-asp-native",
                "queryKeys": [owner],
            })
        })
        .collect::<Vec<_>>();
    let symbols = result
        .facts
        .iter()
        .filter(|fact| owner_paths.contains(&fact.location.path))
        .map(|fact| {
            serde_json::json!({
                "ownerPath": fact.location.path,
                "name": fact.name,
                "qualifiedName": fact.qualified_name,
                "kind": fact.kind,
                "visibility": fact.visibility,
                "symbolId": fact.symbol_id,
                "semanticVariantId": fact.semantic_variant_id,
                "translationUnit": fact.translation_unit,
                "compileContextDigest": fact.compile_context_digest,
                "structuralSelector": fact.location.structural_selector,
                "sourceLocator": format!(
                    "{}:{}:{}",
                    fact.location.path,
                    fact.location.start_line,
                    fact.location.end_line
                ),
                "queryKeys": [
                    fact.name,
                    fact.qualified_name,
                    fact.symbol_id,
                    fact.semantic_variant_id,
                ],
            })
        })
        .collect::<Vec<_>>();
    let dependency_usages = result
        .dependency_usages
        .iter()
        .filter(|dependency| owner_paths.contains(&dependency.owner_path))
        .map(|dependency| {
            let package_name = if dependency.package_name.is_empty() {
                dependency
                    .import_path
                    .rsplit('/')
                    .next()
                    .unwrap_or("unknown")
            } else {
                dependency.package_name.as_str()
            };
            serde_json::json!({
                "ownerPath": dependency.owner_path,
                "translationUnit": dependency.translation_unit,
                "compileContextDigest": dependency.compile_context_digest,
                "semanticVariantId": dependency.semantic_variant_id,
                "packageName": package_name,
                "importPath": dependency.import_path,
                "source": "clang-preprocessor",
                "sourceLocator": dependency.source_locator,
                "queryKeys": dependency.query_keys,
            })
        })
        .collect::<Vec<_>>();
    let compile_contexts = result
        .compile_contexts
        .iter()
        .map(|context| {
            serde_json::json!({
                "translationUnit": context.translation_unit,
                "digest": context.digest,
            })
        })
        .collect::<Vec<_>>();
    let packet = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-structural-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "generationId": generation_id.as_str(),
        "languageId": request.language_id,
        "providerId": request.provider_id,
        "exportMethod": "native-library",
        "projectRoot": request.project_root,
        "rawSourceStored": false,
        "fileHashes": file_hashes,
        "owners": owners,
        "symbols": symbols,
        "dependencyUsages": dependency_usages,
        "compileContexts": compile_contexts,
        "translationUnits": result.translation_units,
    });
    let bytes = serde_json::to_vec(&packet)
        .map_err(|error| format!("failed to encode native structural-index v1 packet: {error}"))?;
    Ok((generation, bytes))
}
