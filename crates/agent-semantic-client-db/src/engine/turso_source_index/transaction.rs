pub(super) const TURSO_SOURCE_INDEX_COLD_WRITE_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TursoSourceIndexWriteStats {
    pub(super) physical_generation_id: String,
    pub(super) changed_owner_count: u32,
    pub(super) removed_owner_count: u32,
    pub(super) posting_write_count: u32,
}
