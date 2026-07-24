//! Strong identifiers used by semantic workspace-scope admission.

impl PartialEq<str> for WorkspaceScopeAnchorKind {
    fn eq(&self, other: &str) -> bool {
        self.0.as_str() == other
    }
}

impl PartialEq<&str> for WorkspaceScopeAnchorKind {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_str() == *other
    }
}

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct WorkspaceScopeProviderId(String);

impl AsRef<str> for WorkspaceScopeProviderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for WorkspaceScopeProviderId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for WorkspaceScopeProviderId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for WorkspaceScopeProviderId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for WorkspaceScopeProviderId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&String> for WorkspaceScopeProviderId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

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

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct WorkspaceScopeLanguageId(String);

impl AsRef<str> for WorkspaceScopeLanguageId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for WorkspaceScopeLanguageId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for WorkspaceScopeLanguageId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for WorkspaceScopeLanguageId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for WorkspaceScopeLanguageId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&String> for WorkspaceScopeLanguageId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

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

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct WorkspaceScopeId(String);

impl AsRef<str> for WorkspaceScopeId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for WorkspaceScopeId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for WorkspaceScopeId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for WorkspaceScopeId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for WorkspaceScopeId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&String> for WorkspaceScopeId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl WorkspaceScopeId {}

impl From<String> for WorkspaceScopeId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct WorkspaceScopePackageId(String);

impl AsRef<str> for WorkspaceScopePackageId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for WorkspaceScopePackageId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for WorkspaceScopePackageId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for WorkspaceScopePackageId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for WorkspaceScopePackageId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&String> for WorkspaceScopePackageId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

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

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct WorkspaceScopeAnchorKind(String);

impl AsRef<str> for WorkspaceScopeAnchorKind {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for WorkspaceScopeAnchorKind {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for WorkspaceScopeAnchorKind {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for WorkspaceScopeAnchorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for WorkspaceScopeAnchorKind {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&String> for WorkspaceScopeAnchorKind {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl From<String> for WorkspaceScopeAnchorKind {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct WorkspaceScopeAnchorSha256(String);

impl AsRef<str> for WorkspaceScopeAnchorSha256 {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for WorkspaceScopeAnchorSha256 {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for WorkspaceScopeAnchorSha256 {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for WorkspaceScopeAnchorSha256 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for WorkspaceScopeAnchorSha256 {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&String> for WorkspaceScopeAnchorSha256 {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl From<String> for WorkspaceScopeAnchorSha256 {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct WorkspaceScopePackageName(String);

impl AsRef<str> for WorkspaceScopePackageName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for WorkspaceScopePackageName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for WorkspaceScopePackageName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for WorkspaceScopePackageName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<&str> for WorkspaceScopePackageName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<&String> for WorkspaceScopePackageName {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl From<String> for WorkspaceScopePackageName {
    fn from(value: String) -> Self {
        Self(value)
    }
}
