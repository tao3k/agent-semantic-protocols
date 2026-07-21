use std::path::{Component, Path, PathBuf};

use agent_semantic_client::source_index::import_language_projection;
use agent_semantic_client_db::ClientDbLanguageProjection;

/// Bounded parser lifecycle request accepted by the ASP facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LanguageProjectionImportRequest {
    owner: PathBuf,
}

impl LanguageProjectionImportRequest {
    /// Parse the explicit, query-free projection-import surface.
    pub(super) fn parse(args: &[String]) -> Result<Option<Self>, String> {
        let Some(command) = args.first().map(String::as_str) else {
            return Ok(None);
        };
        if command != "projection" {
            return Ok(None);
        }
        if args.get(1).map(String::as_str) != Some("import") {
            return Err(projection_import_usage());
        }

        let mut owner = None;
        let mut index = 2;
        while let Some(argument) = args.get(index) {
            match argument.as_str() {
                "--owner" => {
                    let value = args.get(index + 1).ok_or_else(projection_import_usage)?;
                    if owner.replace(PathBuf::from(value)).is_some() {
                        return Err("projection import accepts exactly one --owner".to_string());
                    }
                    index += 2;
                }
                "--workspace" => {
                    args.get(index + 1).ok_or_else(projection_import_usage)?;
                    index += 2;
                }
                _ => return Err(projection_import_usage()),
            }
        }
        let owner = owner.ok_or_else(projection_import_usage)?;
        ensure_relative_owner(&owner)?;
        Ok(Some(Self { owner }))
    }

    /// Produce the only provider invocation this lifecycle action may issue.
    pub(super) fn provider_args(&self, project_root: &Path) -> Vec<String> {
        vec![
            "projection".to_string(),
            self.owner.display().to_string(),
            "--workspace".to_string(),
            project_root.display().to_string(),
            "--json".to_string(),
        ]
    }

    /// Decode, validate, and persist a parser-owned projection receipt.
    pub(super) fn import_output(
        &self,
        language_id: &str,
        project_root: &Path,
        stdout: &[u8],
    ) -> Result<(), String> {
        let stdout = std::str::from_utf8(stdout)
            .map_err(|error| format!("projection import emitted non-UTF-8 JSON: {error}"))?;
        let projection = ClientDbLanguageProjection::from_json(stdout)?;
        if projection.language_id != language_id {
            return Err(format!(
                "projection import language mismatch: requested={language_id} received={}",
                projection.language_id
            ));
        }
        let owner = self.owner.to_string_lossy();
        if !projection.sources.iter().any(|source| source.path == owner) {
            return Err(format!(
                "projection import did not contain requested owner source `{owner}`"
            ));
        }
        let report = import_language_projection(project_root, projection)?;
        let status = if report.reused { "reused" } else { "imported" };
        println!(
            "[projection-import] language={language_id} owner={owner} status={status} parserProcessCount=1 nodeLocatorCount={}",
            report.node_locator_count
        );
        Ok(())
    }

    /// Render a bounded failure that preserves the lifecycle boundary.
    pub(super) fn provider_failure(&self, status_code: Option<i32>, stderr: &[u8]) -> String {
        let stderr = String::from_utf8_lossy(stderr);
        let detail = stderr.trim().chars().take(320).collect::<String>();
        let owner = self.owner.display();
        match (status_code, detail.is_empty()) {
            (Some(code), true) => {
                format!("projection import harness failed: owner={owner} exitCode={code}")
            }
            (Some(code), false) => format!(
                "projection import harness failed: owner={owner} exitCode={code} detail={detail}"
            ),
            (None, true) => format!("projection import harness failed: owner={owner}"),
            (None, false) => {
                format!("projection import harness failed: owner={owner} detail={detail}")
            }
        }
    }
}

fn ensure_relative_owner(owner: &Path) -> Result<(), String> {
    if owner.as_os_str().is_empty()
        || owner.is_absolute()
        || owner
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(
            "projection import --owner must be a non-empty relative source path".to_string(),
        );
    }
    Ok(())
}

fn projection_import_usage() -> String {
    "usage: asp <language> projection import --owner <relative-owner-path> --workspace <root>"
        .to_string()
}
