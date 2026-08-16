#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbeddedAgentAsset {
    pub file_name: &'static str,
    pub contents: &'static [u8],
}

pub fn embedded_agent_assets() -> &'static [EmbeddedAgentAsset] {
    include!(concat!(env!("OUT_DIR"), "/embedded_agent_assets.rs"))
}
