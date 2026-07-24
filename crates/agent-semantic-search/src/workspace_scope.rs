//! Provider-resolved workspace admission before evidence graph construction.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceScopeProviderId(String);

impl WorkspaceScopeProviderId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkspaceScopeProviderId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceScopeLanguageId(String);

impl WorkspaceScopeLanguageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkspaceScopeLanguageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceScopeId(String);

impl WorkspaceScopeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkspaceScopeId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceScopePackageId(String);

impl WorkspaceScopePackageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkspaceScopePackageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceScopeAnchorKind(String);

impl From<String> for WorkspaceScopeAnchorKind {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceScopeAnchorSha256(String);

impl From<String> for WorkspaceScopeAnchorSha256 {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceScopePackageName(String);

impl From<String> for WorkspaceScopePackageName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticWorkspaceAnchor {
    kind: WorkspaceScopeAnchorKind,
    path: PathBuf,
    sha256: WorkspaceScopeAnchorSha256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticWorkspacePackage {
    package_id: WorkspaceScopePackageId,
    name: WorkspaceScopePackageName,
    root: PathBuf,
    manifest_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticWorkspaceScope {
    workspace_id: String,
    language_id: String,
    provider_id: String,
    package_manager: String,
    source_extensions: Vec<String>,
    discovery_root: PathBuf,
    anchors: Vec<SemanticWorkspaceAnchor>,
    packages: Vec<SemanticWorkspacePackage>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticWorkspaceScopeSet {
    scopes: Vec<SemanticWorkspaceScope>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceCandidateAdmission {
    workspace_id: WorkspaceScopeId,
    package_id: WorkspaceScopePackageId,
    language_id: WorkspaceScopeLanguageId,
    provider_id: WorkspaceScopeProviderId,
    canonical_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceCandidateRejection {
    pub reason_kind: &'static str,
    pub detail: String,
}

impl SemanticWorkspaceScope {
    pub fn matches_provider_identity(
        &self,
        provider_id: &WorkspaceScopeProviderId,
        language_id: &WorkspaceScopeLanguageId,
        discovery_root: &Path,
    ) -> bool {
        self.provider_id == provider_id.as_str()
            && self.language_id == language_id.as_str()
            && self.discovery_root == discovery_root
    }

    pub fn from_packet(packet: &Value) -> Result<Self, String> {
        require_text(
            packet,
            "schemaId",
            "agent.semantic-protocols.semantic-workspace-scope",
        )?;
        require_text(packet, "schemaVersion", "1")?;
        let workspace_id = text_field(packet, "workspaceId")?.to_owned();
        let language_id = identifier_field(packet, "languageId")?.to_owned();
        let provider_id = text_field(packet, "providerId")?.to_owned();
        let package_manager = identifier_field(packet, "packageManager")?.to_owned();
        let source_extension_values = packet
            .get("sourceExtensions")
            .and_then(Value::as_array)
            .ok_or_else(|| "workspace scope sourceExtensions must be an array".to_owned())?;
        let mut source_extensions = source_extension_values
            .iter()
            .map(source_extension)
            .collect::<Result<Vec<_>, _>>()?;
        let source_extension_count = source_extensions.len();
        source_extensions.sort();
        source_extensions.dedup();
        if source_extensions.is_empty() {
            return Err("workspace scope sourceExtensions must not be empty".to_owned());
        }
        if source_extensions.len() != source_extension_count {
            return Err("workspace scope sourceExtensions must be unique".to_owned());
        }
        let discovery_root = absolute_path_field(packet, "discoveryRoot")?;
        let anchor_values = packet
            .get("anchors")
            .and_then(Value::as_array)
            .ok_or_else(|| "workspace scope anchors must be an array".to_owned())?;
        let mut anchors = Vec::with_capacity(anchor_values.len());
        for anchor in anchor_values {
            let kind = identifier_field(anchor, "kind")?.to_owned();
            let path = absolute_path_field(anchor, "path")?;
            let sha256 = sha256_field(anchor, "sha256")?.to_owned();
            anchors.push(SemanticWorkspaceAnchor { kind, path, sha256 });
        }
        if anchors.is_empty() {
            return Err("workspace scope must contain at least one anchor".to_owned());
        }
        let package_values = packet
            .get("packages")
            .and_then(Value::as_array)
            .ok_or_else(|| "workspace scope packages must be an array".to_owned())?;
        let admitted_root_values = packet
            .get("admittedRoots")
            .and_then(Value::as_array)
            .ok_or_else(|| "workspace scope admittedRoots must be an array".to_owned())?;
        let admitted_roots = admitted_root_values
            .iter()
            .map(|value| absolute_text_path(value, "admittedRoots[]"))
            .collect::<Result<Vec<_>, _>>()?;
        if admitted_roots.is_empty() {
            return Err("workspace scope admittedRoots must not be empty".to_owned());
        }
        let admitted_root_set = admitted_roots
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        if admitted_root_set.len() != admitted_roots.len() {
            return Err("workspace scope admittedRoots must be unique".to_owned());
        }

        let mut packages = Vec::with_capacity(package_values.len());
        for package in package_values {
            let package_id = text_field(package, "packageId")?.to_owned();
            let name = text_field(package, "name")?.to_owned();
            let package_language = text_field(package, "languageId")?;
            if package_language != language_id {
                return Err(format!(
                    "workspace package {package_id} language {package_language} does not match {language_id}"
                ));
            }
            let root = absolute_path_field(package, "root")?;
            let manifest_path = absolute_path_field(package, "manifestPath")?;
            if !admitted_roots.contains(&root) {
                return Err(format!(
                    "workspace package {package_id} root is absent from admittedRoots"
                ));
            }
            packages.push(SemanticWorkspacePackage {
                package_id,
                name,
                root,
                manifest_path,
            });
        }
        if packages.is_empty() {
            return Err("workspace scope must admit at least one package".to_owned());
        }
        if admitted_roots
            .iter()
            .any(|root| !packages.iter().any(|package| &package.root == root))
        {
            return Err(
                "workspace scope admittedRoots contains an unknown package root".to_owned(),
            );
        }
        let package_ids = packages
            .iter()
            .map(|package| package.package_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if package_ids.len() != packages.len() {
            return Err("workspace scope packageId values must be unique".to_owned());
        }
        let package_roots = packages
            .iter()
            .map(|package| &package.root)
            .collect::<std::collections::BTreeSet<_>>();
        if package_roots.len() != packages.len() {
            return Err("workspace scope package roots must be unique".to_owned());
        }
        let fingerprint = sha256_field(packet, "fingerprint")?.to_owned();
        packages.sort_by(|left, right| {
            right
                .root
                .components()
                .count()
                .cmp(&left.root.components().count())
                .then_with(|| left.package_id.cmp(&right.package_id))
        });
        Ok(Self {
            workspace_id,
            language_id,
            provider_id,
            package_manager,
            source_extensions,
            discovery_root,
            anchors,
            packages,
            fingerprint,
        })
    }

    pub fn admit_candidate(
        &self,
        candidate: &Path,
        language_id: &WorkspaceScopeLanguageId,
    ) -> Result<WorkspaceCandidateAdmission, WorkspaceCandidateRejection> {
        self.admit_candidate_from(&self.discovery_root, candidate, language_id)
    }

    pub fn admit_candidate_from(
        &self,
        candidate_base: &Path,
        candidate: &Path,
        language_id: &WorkspaceScopeLanguageId,
    ) -> Result<WorkspaceCandidateAdmission, WorkspaceCandidateRejection> {
        if language_id.as_str() != self.language_id {
            return Err(WorkspaceCandidateRejection {
                reason_kind: "candidate-language-mismatch",
                detail: format!(
                    "candidate language {} does not match workspace language {}",
                    language_id.as_str(),
                    self.language_id
                ),
            });
        }
        let canonical_path = normalize_absolute(if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            candidate_base.join(candidate)
        });
        let is_anchor = self
            .anchors
            .iter()
            .any(|anchor| anchor.path == canonical_path);
        let package = self
            .packages
            .iter()
            .find(|package| canonical_path.starts_with(&package.root));
        let Some(package) = package else {
            if is_anchor {
                return Ok(WorkspaceCandidateAdmission {
                    workspace_id: self.workspace_id.clone(),
                    package_id: self.workspace_id.clone(),
                    language_id: self.language_id.clone(),
                    provider_id: self.provider_id.clone(),
                    canonical_path,
                });
            }
            return Err(WorkspaceCandidateRejection {
                reason_kind: "candidate-out-of-scope",
                detail: format!(
                    "candidate {} belongs to no provider-admitted package root",
                    canonical_path.display()
                ),
            });
        };
        let candidate_extension = canonical_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{extension}"));
        if !is_anchor
            && !candidate_extension.as_ref().is_some_and(|extension| {
                self.source_extensions
                    .iter()
                    .any(|admitted| admitted == extension)
            })
        {
            return Err(WorkspaceCandidateRejection {
                reason_kind: "candidate-language-mismatch",
                detail: format!(
                    "candidate {} matches neither provider anchors nor sourceExtensions {}",
                    canonical_path.display(),
                    self.source_extensions.join("|")
                ),
            });
        }
        Ok(WorkspaceCandidateAdmission {
            workspace_id: self.workspace_id.clone(),
            package_id: package.package_id.clone(),
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            canonical_path,
        })
    }
}

impl SemanticWorkspaceScopeSet {
    pub fn new(mut scopes: Vec<SemanticWorkspaceScope>) -> Result<Self, String> {
        if scopes.is_empty() {
            return Err("workspace scope set must contain at least one provider scope".to_owned());
        }
        scopes.sort_by(|left, right| {
            left.provider_id
                .cmp(&right.provider_id)
                .then_with(|| left.workspace_id.cmp(&right.workspace_id))
        });
        if scopes.windows(2).any(|pair| {
            pair[0].provider_id == pair[1].provider_id
                && pair[0].workspace_id == pair[1].workspace_id
        }) {
            return Err("workspace scope set contains a duplicate provider workspace".to_owned());
        }
        Ok(Self { scopes })
    }

    pub fn admit_candidate_from(
        &self,
        candidate_base: &Path,
        candidate: &Path,
    ) -> Result<WorkspaceCandidateAdmission, WorkspaceCandidateRejection> {
        let mut admitted = self
            .scopes
            .iter()
            .filter_map(|scope| {
                scope
                    .admit_candidate_from(candidate_base, candidate, &scope.language_id)
                    .ok()
                    .map(|admission| (scope_admission_specificity(scope, &admission), admission))
            })
            .collect::<Vec<_>>();
        admitted.sort_by(|left, right| right.0.cmp(&left.0));
        let Some((specificity, admission)) = admitted.first() else {
            let canonical_path = normalize_absolute(if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                candidate_base.join(candidate)
            });
            let language_claimed = self.scopes.iter().any(|scope| {
                scope
                    .packages
                    .iter()
                    .any(|package| canonical_path.starts_with(&package.root))
                    || scope
                        .anchors
                        .iter()
                        .any(|anchor| anchor.path == canonical_path)
            });
            return Err(WorkspaceCandidateRejection {
                reason_kind: if language_claimed {
                    "candidate-language-mismatch"
                } else {
                    "candidate-out-of-scope"
                },
                detail: if language_claimed {
                    format!(
                        "candidate {} matches no source extension or anchor claimed by its provider scopes",
                        canonical_path.display()
                    )
                } else {
                    format!(
                        "candidate {} belongs to no provider-admitted workspace scope",
                        canonical_path.display()
                    )
                },
            });
        };
        let equally_specific = admitted
            .iter()
            .take_while(|(candidate_specificity, _)| candidate_specificity == specificity)
            .map(|(_, admission)| admission)
            .collect::<Vec<_>>();
        if equally_specific.len() > 1 {
            let providers = equally_specific
                .iter()
                .map(|admission| admission.provider_id.as_str())
                .collect::<Vec<_>>()
                .join("|");
            return Err(WorkspaceCandidateRejection {
                reason_kind: "candidate-provider-ambiguous",
                detail: format!(
                    "candidate {} is claimed at equal specificity by providers {providers}",
                    admission.canonical_path.display()
                ),
            });
        }
        Ok((*admission).clone())
    }
}

fn scope_admission_specificity(
    scope: &SemanticWorkspaceScope,
    admission: &WorkspaceCandidateAdmission,
) -> usize {
    scope
        .packages
        .iter()
        .find(|package| package.package_id == admission.package_id)
        .map(|package| package.root.components().count())
        .unwrap_or_else(|| scope.discovery_root.components().count())
}

fn text_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("workspace scope {field} must be non-empty text"))
}

fn identifier_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    let identifier = text_field(value, field)?;
    let mut bytes = identifier.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!(
            "workspace scope {field} must be a lowercase provider-owned identifier"
        ));
    }
    Ok(identifier)
}

fn source_extension(value: &Value) -> Result<String, String> {
    let extension = value
        .as_str()
        .filter(|value| value.starts_with('.') && value.len() > 1)
        .ok_or_else(|| "workspace scope sourceExtensions[] must start with a dot".to_owned())?;
    let mut suffix = extension.bytes().skip(1);
    if !suffix
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        || !suffix
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
    {
        return Err(format!(
            "workspace scope source extension is invalid: {extension}"
        ));
    }
    Ok(extension.to_owned())
}

fn require_text(value: &Value, field: &str, expected: &str) -> Result<(), String> {
    let actual = text_field(value, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "workspace scope {field} must be {expected}, found {actual}"
        ))
    }
}

fn sha256_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    let rendered = text_field(value, field)?;
    let digest = rendered
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("workspace scope {field} must use sha256:<hex>"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "workspace scope {field} must contain 64 lowercase hexadecimal digits"
        ));
    }
    Ok(rendered)
}

fn absolute_path_field(value: &Value, field: &str) -> Result<PathBuf, String> {
    absolute_text_path(
        value
            .get(field)
            .ok_or_else(|| format!("workspace scope {field} is required"))?,
        field,
    )
}

fn absolute_text_path(value: &Value, field: &str) -> Result<PathBuf, String> {
    let text = value
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("workspace scope {field} must be non-empty text"))?;
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(format!("workspace scope {field} must be an absolute path"));
    }
    Ok(normalize_absolute(path))
}

fn normalize_absolute(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
