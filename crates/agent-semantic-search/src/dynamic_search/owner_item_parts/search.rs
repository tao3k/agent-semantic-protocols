#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::dynamic_search) struct OwnerItemMatch {
    pub(in crate::dynamic_search) start: usize,
    pub(in crate::dynamic_search) end: usize,
    pub(in crate::dynamic_search) kind: String,
    pub(in crate::dynamic_search) term: String,
    pub(in crate::dynamic_search) rank: u8,
}
