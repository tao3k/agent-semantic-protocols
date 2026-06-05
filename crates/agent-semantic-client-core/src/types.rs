//! Shared semantic scalar types used by the agent semantic client.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::time::Duration;

macro_rules! semantic_string_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Create a typed semantic string.
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Return the wire string.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Return true when the wire string is empty.
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }

            /// Unwrap the typed semantic string.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

semantic_string_type!(
    /// Language provider id such as `rust`, `typescript`, or `python`.
    LanguageId
);
semantic_string_type!(
    /// Provider implementation id such as `rs-harness`.
    ProviderId
);
semantic_string_type!(
    /// JSON schema id carried by a client envelope.
    SemanticSchemaId
);
semantic_string_type!(
    /// JSON schema version carried by a client envelope.
    SemanticSchemaVersion
);
semantic_string_type!(
    /// Protocol id carried by a client envelope.
    SemanticProtocolId
);
semantic_string_type!(
    /// Protocol version carried by a client envelope.
    SemanticProtocolVersion
);
semantic_string_type!(
    /// Provider-owned cache generation id.
    CacheGenerationId
);
semantic_string_type!(
    /// Provider-advertised export method such as `search/prime`.
    CacheExportMethod
);
semantic_string_type!(
    /// Provider-owned cache artifact id, relative to the protocol artifact root.
    CacheArtifactId
);
semantic_string_type!(
    /// Compact artifact id produced or referenced by the client.
    CompactArtifactId
);
semantic_string_type!(
    /// Stable fingerprint of the normalized tree-sitter-compatible query AST/ABI.
    SyntaxQueryAstAbiFingerprint
);
semantic_string_type!(
    /// Tree-sitter grammar id used by a syntax-query packet.
    SyntaxQueryGrammarId
);
semantic_string_type!(
    /// Grammar profile version used by a syntax-query packet.
    SyntaxQueryGrammarProfileVersion
);
semantic_string_type!(
    /// Selector component used by a syntax-query cache identity.
    SyntaxQuerySelector
);
semantic_string_type!(
    /// UTF-8 cache path serialized in client receipts and manifests.
    ClientCachePath
);
semantic_string_type!(
    /// Observed SQLite journal mode for the local client DB.
    ClientDbJournalMode
);

impl ClientCachePath {
    /// Convert a filesystem path into the client wire path string.
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        Self(path.display().to_string())
    }
}

/// Byte count captured for provider stdout and stderr.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteCount(u64);

impl ByteCount {
    /// Create a byte count from an exact `u64`.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Create a byte count from a buffer length.
    #[must_use]
    pub fn from_len(value: usize) -> Self {
        Self(value.min(u64::MAX as usize) as u64)
    }

    /// Return the numeric byte count.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ByteCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Elapsed time in milliseconds for local provider execution.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElapsedMillis(u64);

impl ElapsedMillis {
    /// Create an elapsed millisecond count from an exact `u64`.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Saturating conversion from a `Duration`.
    #[must_use]
    pub fn from_duration(elapsed: Duration) -> Self {
        Self(elapsed.as_millis().min(u128::from(u64::MAX)) as u64)
    }

    /// Return the numeric millisecond count.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ElapsedMillis {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Cache state observed by the client route or a provider cache generation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheStatus {
    Cold,
    WarmProvider,
    Hit,
    Miss,
    Stale,
    Invalidated,
    Disabled,
}

impl CacheStatus {
    /// Return the canonical wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::WarmProvider => "warm-provider",
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::Stale => "stale",
            Self::Invalidated => "invalidated",
            Self::Disabled => "disabled",
        }
    }
}

/// SQLite client DB availability state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientDbStatus {
    Missing,
    Present,
    Invalid,
}

impl ClientDbStatus {
    /// Return the compact line-protocol status spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Present => "present",
            Self::Invalid => "invalid",
        }
    }
}
