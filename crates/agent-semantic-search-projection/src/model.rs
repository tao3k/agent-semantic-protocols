use serde::{Deserialize, Serialize};

use super::SearchProjectionError;

pub const SEARCH_PROJECTION_REQUEST_SCHEMA_ID: &str = "asp.search-projection-request.v1";
pub const RENDERED_SEARCH_PROJECTION_SCHEMA_ID: &str = "asp.rendered-search-projection.v1";
pub const SEARCH_PROJECTION_SCHEMA_VERSION: &str = "v1";

macro_rules! search_projection_text_v1 {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

search_projection_text_v1!(SearchProjectionSchemaIdV1);
search_projection_text_v1!(SearchProjectionSchemaVersionV1);
search_projection_text_v1!(SearchProjectionIdV1);

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchProjectionDensityV1 {
    Terse,
    #[default]
    Standard,
    Expanded,
}

impl SearchProjectionDensityV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terse => "terse",
            Self::Standard => "standard",
            Self::Expanded => "expanded",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchProjectionRequestV1 {
    pub(crate) schema_id: SearchProjectionSchemaIdV1,
    pub(crate) schema_version: SearchProjectionSchemaVersionV1,
    pub(crate) projection_id: SearchProjectionIdV1,
    pub(crate) density: SearchProjectionDensityV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_rows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_bytes: Option<usize>,
}

impl SearchProjectionRequestV1 {
    pub fn new(projection_id: impl Into<String>, density: SearchProjectionDensityV1) -> Self {
        Self {
            schema_id: SEARCH_PROJECTION_REQUEST_SCHEMA_ID.to_string().into(),
            schema_version: SEARCH_PROJECTION_SCHEMA_VERSION.to_string().into(),
            projection_id: projection_id.into().into(),
            density,
            max_rows: None,
            max_bytes: None,
        }
    }

    pub fn validate(&self) -> Result<(), SearchProjectionError> {
        if self.schema_id.as_str() != SEARCH_PROJECTION_REQUEST_SCHEMA_ID {
            return Err(SearchProjectionError::InvalidRequest(
                "schemaId must be asp.search-projection-request.v1".to_string(),
            ));
        }
        if self.schema_version.as_str() != SEARCH_PROJECTION_SCHEMA_VERSION {
            return Err(SearchProjectionError::InvalidRequest(
                "schemaVersion must be v1".to_string(),
            ));
        }
        if self.projection_id.as_str().trim().is_empty() {
            return Err(SearchProjectionError::InvalidRequest(
                "projectionId must not be empty".to_string(),
            ));
        }
        if self.max_rows == Some(0) {
            return Err(SearchProjectionError::InvalidRequest(
                "maxRows must be greater than zero".to_string(),
            ));
        }
        if self.max_bytes == Some(0) {
            return Err(SearchProjectionError::InvalidRequest(
                "maxBytes must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderedSearchProjectionV1 {
    pub(crate) schema_id: SearchProjectionSchemaIdV1,
    pub(crate) schema_version: SearchProjectionSchemaVersionV1,
    pub(crate) projection_id: SearchProjectionIdV1,
    pub(crate) density: SearchProjectionDensityV1,
    pub(crate) semantic_digest: String,
    pub(crate) content_type: String,
    pub(crate) content: String,
}
