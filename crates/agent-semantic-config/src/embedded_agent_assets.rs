//! Build-generated agent prompt assets embedded in the Config crate.

/// One immutable agent asset compiled into the current binary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedAgentAsset {
    /// Canonical file name used when materializing the asset.
    pub file_name: &'static str,
    /// Exact content bytes generated from the source asset.
    pub contents: &'static [u8],
}

/// Returns every agent asset embedded by the Config build transaction.
pub fn embedded_agent_assets() -> &'static [EmbeddedAgentAsset] {
    include!(concat!(env!("OUT_DIR"), "/embedded_agent_assets.rs"))
}
