use std::path::Path;

pub(super) fn ensure_provider_source_scope_fixture(
    root: &Path,
    manifest: &agent_semantic_hook::ProviderManifest,
) {
    let source = manifest.source();
    if source
        .default_config_files
        .iter()
        .any(|path| root.join(path).is_file())
        || source.default_source_roots.iter().any(|source_root| {
            directory_contains_registered_source(
                &root.join(source_root),
                &source.default_extensions,
                &super::state_home(root),
            )
        })
    {
        return;
    }
    let Some(extension) = source.default_extensions.first() else {
        return;
    };
    let source_root = source
        .default_source_roots
        .first()
        .map_or_else(|| root.to_path_buf(), |path| root.join(path));
    std::fs::create_dir_all(&source_root).expect("create provider source-scope fixture root");
    std::fs::write(
        source_root.join(format!(
            "asp_scope_fixture_{}{}",
            manifest.language_id(),
            extension
        )),
        "",
    )
    .expect("write provider source-scope fixture");
}

fn directory_contains_registered_source(
    directory: &Path,
    extensions: &[String],
    excluded_state_home: &Path,
) -> bool {
    if !directory.is_dir() || directory.starts_with(excluded_state_home) {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if directory_contains_registered_source(&path, extensions, excluded_state_home) {
                return true;
            }
            continue;
        }
        if path.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    extensions
                        .iter()
                        .any(|candidate| candidate.trim_start_matches('.') == extension)
                })
        {
            return true;
        }
    }
    false
}
