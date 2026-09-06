// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Typed semantic identifiers shared by content-bound Runtime contracts.

use std::fmt;

use serde::Deserialize;
use serde::Serialize;

macro_rules! semantic_text_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Returns the identifier without transferring ownership.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Transfers the validated identifier text to the caller.
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
                Self(value.to_owned())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.as_str() == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

semantic_text_id!(
    /// Language authority selected for a content-bound operation.
    LanguageIdV1
);
semantic_text_id!(
    /// Provider authority selected for a content-bound operation.
    ProviderIdV1
);
semantic_text_id!(
    /// Host platform that produced a session observation.
    HostPlatformV1
);
semantic_text_id!(
    /// One explicit Host session identity in a lineage.
    HostSessionIdV1
);
semantic_text_id!(
    /// Provider-defined semantic relation vocabulary item.
    ProviderRelationKindV1
);
semantic_text_id!(
    /// Canonical owner or parser-item endpoint identity.
    ProviderRelationEndpointIdV1
);
semantic_text_id!(
    /// Semantic projection operation kind.
    SemanticProjectionKindV1
);
semantic_text_id!(
    /// Root selector of a bounded semantic projection.
    SemanticProjectionRootSelectorV1
);
semantic_text_id!(
    /// Content-addressed reference to projection evidence.
    ProjectionEvidenceContextRefV1
);
semantic_text_id!(
    /// Stable schema identifier used by a typed semantic payload.
    SchemaIdV1
);
semantic_text_id!(
    /// Prefixed BLAKE3 content digest at a Runtime contract boundary.
    Blake3DigestV1
);

/// Closed endpoint domain for provider-projected relations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRelationEndpointKindV1 {
    /// Endpoint names a source owner.
    Owner,
    /// Endpoint names a parser-owned item.
    Item,
}

impl ProviderRelationEndpointKindV1 {
    /// Returns the stable wire discriminator for this endpoint kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Item => "item",
        }
    }
}

impl TryFrom<&str> for ProviderRelationEndpointKindV1 {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "owner" => Ok(Self::Owner),
            "item" => Ok(Self::Item),
            other => Err(format!(
                "unsupported provider relation endpoint kind: {other}"
            )),
        }
    }
}

impl fmt::Display for ProviderRelationEndpointKindV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
