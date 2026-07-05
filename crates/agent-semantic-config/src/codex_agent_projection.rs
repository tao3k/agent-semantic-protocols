use std::fs;
use std::path::{Path, PathBuf};

pub fn write_codex_dynamic_model(config_path: &Path, model: &str) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut value = if config_path.exists() {
        let text = fs::read_to_string(config_path)
            .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
        toml::from_str::<toml::Value>(&text)
            .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?
    } else {
        toml::Value::Table(Default::default())
    };
    let root = value
        .as_table_mut()
        .ok_or_else(|| format!("{} must contain a TOML table", config_path.display()))?;
    let platform = root
        .entry("platform".to_string())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| format!("{}.platform must be a TOML table", config_path.display()))?;
    let codex = platform
        .entry("codex".to_string())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| {
            format!(
                "{}.platform.codex must be a TOML table",
                config_path.display()
            )
        })?;
    let models = codex
        .entry("models".to_string())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| {
            format!(
                "{}.platform.codex.models must be a TOML table",
                config_path.display()
            )
        })?;
    models.insert(
        "primary".to_string(),
        toml::Value::String(model.to_string()),
    );
    write_toml_value(config_path, &value)
}

pub fn update_asp_codex_agent_sources_and_symlink_projections(
    asp_agents_dir: &Path,
    codex_agents_dir: &Path,
    model: &str,
    updated_agent_configs: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if !asp_agents_dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(codex_agents_dir)
        .map_err(|error| format!("failed to create {}: {error}", codex_agents_dir.display()))?;
    for entry in fs::read_dir(asp_agents_dir)
        .map_err(|error| format!("failed to read {}: {error}", asp_agents_dir.display()))?
    {
        let entry = entry
            .map_err(|error| format!("failed to read {}: {error}", asp_agents_dir.display()))?;
        let source_path = entry.path();
        if !source_path.is_file() {
            continue;
        }
        let Some(file_name) = source_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
        else {
            continue;
        };
        let Some(projection_stem) = file_name.strip_suffix("_codex.toml") else {
            continue;
        };
        update_agent_model_file(&source_path, model)?;
        updated_agent_configs.push(source_path.clone());

        let projection_path = codex_agents_dir.join(format!("{projection_stem}.toml"));
        replace_with_symlink(&source_path, &projection_path)?;
        updated_agent_configs.push(projection_path);
    }
    Ok(())
}

fn update_agent_model_file(path: &Path, model: &str) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut value = toml::from_str::<toml::Value>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let table = value
        .as_table_mut()
        .ok_or_else(|| format!("{} must contain a TOML table", path.display()))?;
    table.insert("model".to_string(), toml::Value::String(model.to_string()));
    write_toml_value(path, &value)
}

fn write_toml_value(path: &Path, value: &toml::Value) -> Result<(), String> {
    let mut text = toml::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    fs::write(path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

#[cfg(unix)]
fn replace_with_symlink(source: &Path, projection: &Path) -> Result<(), String> {
    if fs::symlink_metadata(projection).is_ok() {
        fs::remove_file(projection)
            .map_err(|error| format!("failed to remove {}: {error}", projection.display()))?;
    }
    std::os::unix::fs::symlink(source, projection).map_err(|error| {
        format!(
            "failed to symlink {} to {}: {error}",
            projection.display(),
            source.display()
        )
    })
}

#[cfg(not(unix))]
fn replace_with_symlink(source: &Path, projection: &Path) -> Result<(), String> {
    if fs::symlink_metadata(projection).is_ok() {
        fs::remove_file(projection)
            .map_err(|error| format!("failed to remove {}: {error}", projection.display()))?;
    }
    fs::copy(source, projection).map(|_| ()).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            projection.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn codex_agent_projection_is_symlink_and_does_not_truncate_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let asp_agents = temp.path().join("asp-agents");
        let codex_agents = temp.path().join("codex-agents");
        fs::create_dir_all(&asp_agents).expect("create asp agents");
        let source = asp_agents.join("asp-explorer_codex.toml");
        fs::write(
            &source,
            "name = \"asp_explorer\"\nmodel = \"gpt-5.4-mini\"\nsandbox_mode = \"read-only\"\n",
        )
        .expect("write source");
        fs::create_dir_all(&codex_agents).expect("create codex agents");
        std::os::unix::fs::symlink(&source, codex_agents.join("asp-explorer.toml"))
            .expect("seed symlink");

        let mut updated = Vec::new();
        update_asp_codex_agent_sources_and_symlink_projections(
            &asp_agents,
            &codex_agents,
            "gpt-5.5",
            &mut updated,
        )
        .expect("update projection");

        let source_text = fs::read_to_string(&source).expect("read source");
        assert!(source_text.contains("model = \"gpt-5.5\""));
        assert!(source_text.contains("sandbox_mode = \"read-only\""));
        let projection = codex_agents.join("asp-explorer.toml");
        assert_eq!(fs::read_link(&projection).expect("read link"), source);
    }
}
