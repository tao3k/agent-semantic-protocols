/// Binary contract discriminator for an immutable resident generation that
/// includes the provider relation graph in its content identity.
pub const WORKSPACE_MEMORY_GENERATION_SEGMENT_MAGIC: &[u8; 16] = b"ASPWSMEMORYRELV1";

pub fn has_current_workspace_memory_generation_contract(bytes: &[u8]) -> bool {
    bytes.starts_with(WORKSPACE_MEMORY_GENERATION_SEGMENT_MAGIC)
}
