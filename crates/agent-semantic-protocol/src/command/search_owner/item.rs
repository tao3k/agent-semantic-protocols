#[derive(Debug)]
pub(in crate::command) struct OwnerItem {
    pub(super) name: String,
    pub(super) kind: &'static str,
    pub(super) start_line: usize,
    pub(super) end_line: usize,
}

impl OwnerItem {
    pub(in crate::command) fn name(&self) -> &str {
        &self.name
    }

    pub(in crate::command) fn kind(&self) -> &str {
        self.kind
    }

    pub(in crate::command) fn start_line(&self) -> usize {
        self.start_line
    }

    pub(in crate::command) fn end_line(&self) -> usize {
        self.end_line
    }
}
