//! Stable identity of the ASP Client Protocol and its root wire schemas.

/// Stable protocol family identity shared by every client frame and catalog.
pub const CLIENT_PROTOCOL_ID: &str = "agent.semantic-protocols.client";
/// Stable protocol version implemented by this crate.
pub const CLIENT_PROTOCOL_VERSION: &str = "1";
/// Schema identity of the root client-frame union.
pub const CLIENT_FRAME_SCHEMA_ID: &str = "agent.semantic-protocols.client.frame";
/// Schema identity of the client method catalog.
pub const CLIENT_CATALOG_SCHEMA_ID: &str = "agent.semantic-protocols.client.protocol-catalog";
/// Shared version of the protocol JSON schemas.
pub const SCHEMA_VERSION: &str = "1";
