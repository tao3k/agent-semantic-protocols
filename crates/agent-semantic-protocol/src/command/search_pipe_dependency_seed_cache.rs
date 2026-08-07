//! Persistent dependency seed cache for graph-turbo search packets.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    provider_process::{provider_invocation_with_profile, run_provider_command_with_stdin},
    search_pipe_model::Candidate,
    search_pipe_provider_facts::ProviderGraphFactsContext,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ProviderDependencyTopologyFact {
    pub(super) owner_path: String,
    pub(super) dependency: String,
    pub(super) version: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct CachedDependencyFacts {
    pub(super) cache_status: &'static str,
    pub(super) topology_source: &'static str,
    pub(super) facts: Vec<ProviderDependencyTopologyFact>,
}

struct DependencySeedCacheRecord {
    fingerprint: String,
    sources: Vec<DependencySeedSource>,
    facts: Vec<ProviderDependencyTopologyFact>,
}

struct DependencySeedSource {
    kind: String,
    path: String,
    sha256: String,
}

pub(super) async fn collect_cached_manifest_dependency_facts(
    language_id: &str,
    project_root: &Path,
    cache_home: &Path,
    provider_context: Option<&ProviderGraphFactsContext<'_>>,
) -> Result<CachedDependencyFacts, String> {
    let context = provider_context.ok_or_else(|| {
        format!(
            "dependency topology requires an activated {language_id} provider with parser-owned manifest facts"
        )
    })?;
    if !context.provider.search_capabilities.dependency_topology {
        return Err(format!(
            "activated {language_id} provider does not declare parser-owned dependency topology"
        ));
    }
    collect_provider_dependency_topology_facts(language_id, project_root, cache_home, context).await
}

pub(super) async fn collect_cached_dependency_facts(
    language_id: &str,
    project_root: &Path,
    cache_home: &Path,
    provider_context: Option<&ProviderGraphFactsContext<'_>>,
    _query: Option<&str>,
    _candidates: &[Candidate],
) -> Result<CachedDependencyFacts, String> {
    collect_cached_manifest_dependency_facts(
        language_id,
        project_root,
        cache_home,
        provider_context,
    )
    .await
}

async fn collect_provider_dependency_topology_facts(
    language_id: &str,
    project_root: &Path,
    cache_home: &Path,
    context: &ProviderGraphFactsContext<'_>,
) -> Result<CachedDependencyFacts, String> {
    let cache_path = dependency_seed_cache_path(cache_home, language_id);
    if let Some(facts) = read_current_dependency_seed_cache(&cache_path, project_root) {
        return Ok(CachedDependencyFacts {
            cache_status: "hit",
            topology_source: "provider-owned",
            facts,
        });
    }
    if let Some(fingerprint) =
        provider_dependency_topology_metadata_fingerprint(language_id, project_root, context).await
        && let Some(record) = read_dependency_seed_cache(&cache_path, &fingerprint)
        && !record.sources.is_empty()
    {
        return Ok(CachedDependencyFacts {
            cache_status: "hit",
            topology_source: "provider-owned",
            facts: record.facts,
        });
    }
    let invocation =
        provider_dependency_topology_invocation(context, project_root).map_err(|error| {
            format!(
                "failed to build parser-owned {language_id} dependency topology invocation: {error}"
            )
        })?;
    let output = run_provider_command_with_stdin(
        language_id,
        context.provider,
        &invocation,
        project_root,
        Vec::new(),
    )
    .await
    .map_err(|error| {
        format!("parser-owned {language_id} dependency topology invocation failed: {error}")
    })?;
    if !output.status.success() {
        return Err(format!(
            "parser-owned {language_id} dependency topology provider failed: status={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(output.stderr.as_ref()),
        ));
    }
    let (fingerprint, sources, facts) =
        provider_dependency_facts_from_stdout(output.stdout.as_ref()).ok_or_else(|| {
            format!(
                "parser-owned {language_id} dependency topology returned no typed dependency packet"
            )
        })?;
    if let Some(record) = read_dependency_seed_cache(&cache_path, &fingerprint) {
        if record.sources.is_empty() && !sources.is_empty() {
            write_dependency_seed_cache(&cache_path, &fingerprint, &sources, &record.facts);
        }
        return Ok(CachedDependencyFacts {
            cache_status: "hit",
            topology_source: "provider-owned",
            facts: record.facts,
        });
    }
    write_dependency_seed_cache(&cache_path, &fingerprint, &sources, &facts);
    Ok(CachedDependencyFacts {
        cache_status: "miss",
        topology_source: "provider-owned",
        facts,
    })
}

async fn provider_dependency_topology_metadata_fingerprint(
    language_id: &str,
    project_root: &Path,
    context: &ProviderGraphFactsContext<'_>,
) -> Option<String> {
    if !context
        .provider
        .search_capabilities
        .dependency_topology_metadata
    {
        return None;
    }
    let invocation =
        provider_dependency_topology_metadata_invocation(context, project_root).ok()?;
    let output = run_provider_command_with_stdin(
        language_id,
        context.provider,
        &invocation,
        project_root,
        Vec::new(),
    )
    .await
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = provider_dependency_topology_json(output.stdout.as_ref())?;
    provider_dependency_cache_fingerprint_from_value(&value)
}

fn provider_dependency_topology_metadata_invocation(
    context: &ProviderGraphFactsContext<'_>,
    project_root: &Path,
) -> Result<Vec<String>, String> {
    let template = context
        .provider
        .routes
        .dependency_topology_metadata
        .as_ref()
        .ok_or_else(|| "provider did not declare dependencyTopologyMetadata route".to_string())?;
    let argv = template
        .argv
        .iter()
        .map(|arg| {
            arg.replace(
                "{workspace}",
                &project_root.display().to_string().replace('\\', "/"),
            )
        })
        .collect::<Vec<_>>();
    let Some((program, forwarded)) = argv.split_first() else {
        return Err("provider dependencyTopologyMetadata route argv is empty".to_string());
    };
    if program == &context.provider.binary {
        provider_invocation_with_profile(context.profiles, context.provider, forwarded)
    } else {
        Ok(argv)
    }
}

fn provider_dependency_topology_invocation(
    context: &ProviderGraphFactsContext<'_>,
    project_root: &Path,
) -> Result<Vec<String>, String> {
    let template = context
        .provider
        .routes
        .dependency_topology
        .as_ref()
        .ok_or_else(|| "provider did not declare dependencyTopology route".to_string())?;
    let argv = template
        .argv
        .iter()
        .map(|arg| {
            arg.replace(
                "{workspace}",
                &project_root.display().to_string().replace('\\', "/"),
            )
        })
        .collect::<Vec<_>>();
    let Some((program, forwarded)) = argv.split_first() else {
        return Err("provider dependencyTopology route argv is empty".to_string());
    };
    if program == &context.provider.binary {
        provider_invocation_with_profile(context.profiles, context.provider, forwarded)
    } else {
        Ok(argv)
    }
}

fn provider_dependency_facts_from_stdout(
    stdout: &[u8],
) -> Option<(
    String,
    Vec<DependencySeedSource>,
    Vec<ProviderDependencyTopologyFact>,
)> {
    let value = provider_dependency_topology_json(stdout)?;
    provider_dependency_facts_from_value(&value)
}

fn provider_dependency_topology_json(stdout: &[u8]) -> Option<Value> {
    if let Ok(value) = serde_json::from_slice::<Value>(stdout) {
        return Some(value);
    }
    let start = stdout.iter().position(|byte| *byte == b'{')?;
    let end = stdout.iter().rposition(|byte| *byte == b'}')?;
    if end <= start {
        return None;
    }
    serde_json::from_slice::<Value>(&stdout[start..=end]).ok()
}

fn provider_dependency_facts_from_value(
    value: &Value,
) -> Option<(
    String,
    Vec<DependencySeedSource>,
    Vec<ProviderDependencyTopologyFact>,
)> {
    if value.get("packetKind").and_then(Value::as_str) != Some("dependency-topology") {
        return None;
    }
    let fingerprint = provider_dependency_cache_fingerprint_from_value(value)
        .or_else(|| value.get("fingerprint")?.as_str().map(str::to_string))?;
    let sources = provider_dependency_sources_from_value(value);
    let nodes = value.get("graph")?.get("nodes")?.as_array()?;
    let edges = value.get("graph")?.get("edges")?.as_array()?;
    let mut version_by_id = std::collections::HashMap::new();
    for node in nodes {
        if node.get("kind").and_then(Value::as_str) != Some("dependency-version") {
            continue;
        }
        let Some(id) = node.get("id").and_then(Value::as_str) else {
            continue;
        };
        let version = node
            .get("fields")
            .and_then(|fields| fields.get("version"))
            .and_then(Value::as_str)
            .or_else(|| node.get("value").and_then(Value::as_str))
            .map(str::to_string);
        if let Some(version) = version {
            version_by_id.insert(id.to_string(), version);
        }
    }
    let mut version_target_by_dependency = std::collections::HashMap::new();
    for edge in edges {
        if edge.get("relation").and_then(Value::as_str) != Some("version_locked") {
            continue;
        }
        let (Some(source), Some(target)) = (
            edge.get("source").and_then(Value::as_str),
            edge.get("target").and_then(Value::as_str),
        ) else {
            continue;
        };
        if let Some(version) = version_by_id.get(target) {
            version_target_by_dependency.insert(source.to_string(), version.clone());
        }
    }
    let mut seen = HashSet::new();
    let mut facts = Vec::new();
    for node in nodes {
        if node.get("kind").and_then(Value::as_str) != Some("dependency") {
            continue;
        }
        let dependency = node
            .get("fields")
            .and_then(|fields| fields.get("dependencyName"))
            .and_then(Value::as_str)
            .or_else(|| node.get("value").and_then(Value::as_str))?
            .to_string();
        let owner_path = node
            .get("fields")
            .and_then(|fields| fields.get("manifestPath"))
            .and_then(Value::as_str)
            .or_else(|| node.get("path").and_then(Value::as_str))
            .unwrap_or("dependency-topology")
            .to_string();
        let id = node.get("id").and_then(Value::as_str).unwrap_or_default();
        let version = version_target_by_dependency.get(id).cloned();
        let key = format!("{owner_path}:{dependency}:manifest");
        if seen.insert(key) {
            facts.push(ProviderDependencyTopologyFact {
                owner_path,
                dependency,
                version,
            });
        }
    }
    Some((fingerprint, sources, facts))
}

fn provider_dependency_sources_from_value(value: &Value) -> Vec<DependencySeedSource> {
    let Some(sources) = value.get("sources") else {
        return Vec::new();
    };
    let mut receipts = Vec::new();
    push_dependency_source_receipts(sources, "manifests", "manifest", &mut receipts);
    push_dependency_source_receipts(sources, "lockfiles", "lockfile", &mut receipts);
    receipts
}

fn push_dependency_source_receipts(
    sources: &Value,
    field: &str,
    kind: &str,
    receipts: &mut Vec<DependencySeedSource>,
) {
    let Some(values) = sources.get(field).and_then(Value::as_array) else {
        return;
    };
    for value in values {
        let (Some(path), Some(sha256)) = (
            value.get("path").and_then(Value::as_str),
            value.get("sha256").and_then(Value::as_str),
        ) else {
            continue;
        };
        receipts.push(DependencySeedSource {
            kind: kind.to_string(),
            path: path.to_string(),
            sha256: sha256.to_string(),
        });
    }
}

fn provider_dependency_cache_fingerprint_from_value(value: &Value) -> Option<String> {
    match value.get("packetKind").and_then(Value::as_str)? {
        "dependency-topology" | "dependency-topology-metadata" => {
            dependency_cache_key_fingerprint(value)
                .or_else(|| value.get("fingerprint")?.as_str().map(str::to_string))
        }
        _ => None,
    }
}

fn dependency_cache_key_fingerprint(value: &Value) -> Option<String> {
    let cache_key = value.get("cacheKey")?;
    let language_id = cache_key.get("languageId")?.as_str()?;
    let package_manager = cache_key.get("packageManager")?.as_str()?;
    let project_package_name = cache_key.get("projectPackageName")?.as_str()?;
    let manifest_hash = cache_key.get("manifestHash")?.as_str()?;
    let lockfile_hash = cache_key.get("lockfileHash")?.as_str()?;
    let mut hasher = Sha256::new();
    hasher.update(language_id.as_bytes());
    hasher.update([0]);
    hasher.update(package_manager.as_bytes());
    hasher.update([0]);
    hasher.update(project_package_name.as_bytes());
    hasher.update([0]);
    hasher.update(manifest_hash.as_bytes());
    hasher.update([0]);
    hasher.update(lockfile_hash.as_bytes());
    Some(format!("sha256:{:x}", hasher.finalize()))
}

fn read_current_dependency_seed_cache(
    path: &Path,
    project_root: &Path,
) -> Option<Vec<ProviderDependencyTopologyFact>> {
    let record = parse_dependency_seed_cache(path)?;
    if record.sources.is_empty() {
        return None;
    }
    if !record
        .sources
        .iter()
        .all(|source| dependency_seed_source_current(project_root, source))
    {
        return None;
    }
    Some(record.facts)
}

fn read_dependency_seed_cache(path: &Path, fingerprint: &str) -> Option<DependencySeedCacheRecord> {
    let record = parse_dependency_seed_cache(path)?;
    if record.fingerprint != fingerprint {
        return None;
    }
    Some(record)
}

fn parse_dependency_seed_cache(path: &Path) -> Option<DependencySeedCacheRecord> {
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header = lines.next()?;
    let fingerprint = header.strip_prefix("fingerprint\t")?.to_string();
    let mut sources = Vec::new();
    let mut facts = Vec::new();
    for line in lines {
        let mut parts = line.split('\t');
        match parts.next()? {
            "source" => {
                let kind = parts.next()?.to_string();
                let path = parts.next()?.to_string();
                let sha256 = parts.next()?.to_string();
                sources.push(DependencySeedSource { kind, path, sha256 });
            }
            "fact" => {
                let owner_path = parts.next()?.to_string();
                let dependency = parts.next()?.to_string();
                let version = match parts.next()? {
                    "" => None,
                    value => Some(value.to_string()),
                };
                if parts.next()? != "manifest" {
                    continue;
                }
                facts.push(ProviderDependencyTopologyFact {
                    owner_path,
                    dependency,
                    version,
                });
            }
            _ => {}
        }
    }
    Some(DependencySeedCacheRecord {
        fingerprint,
        sources,
        facts,
    })
}

fn write_dependency_seed_cache(
    path: &Path,
    fingerprint: &str,
    sources: &[DependencySeedSource],
    facts: &[ProviderDependencyTopologyFact],
) {
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let mut text = format!("fingerprint\t{fingerprint}\n");
    for source in sources {
        text.push_str("source\t");
        text.push_str(&source.kind);
        text.push('\t');
        text.push_str(&source.path);
        text.push('\t');
        text.push_str(&source.sha256);
        text.push('\n');
    }
    for fact in facts {
        text.push_str("fact\t");
        text.push_str(&fact.owner_path);
        text.push('\t');
        text.push_str(&fact.dependency);
        text.push('\t');
        text.push_str(fact.version.as_deref().unwrap_or(""));
        text.push_str("\tmanifest\n");
    }
    let _ = fs::write(path, text);
}

fn dependency_seed_source_current(project_root: &Path, source: &DependencySeedSource) -> bool {
    if !matches!(source.kind.as_str(), "manifest" | "lockfile") {
        return false;
    }
    let relative_path = Path::new(&source.path);
    if relative_path.is_absolute() {
        return false;
    }
    sha256_file(&project_root.join(relative_path)).as_deref() == Some(source.sha256.as_str())
}

fn sha256_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(format!("sha256:{:x}", hasher.finalize()))
}

fn dependency_seed_cache_path(cache_home: &Path, language_id: &str) -> PathBuf {
    cache_home
        .join("agent-semantic-protocol")
        .join("search")
        .join("dependency-seeds")
        .join(format!("{}.tsv", safe_cache_key(language_id)))
}

fn safe_cache_key(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}
