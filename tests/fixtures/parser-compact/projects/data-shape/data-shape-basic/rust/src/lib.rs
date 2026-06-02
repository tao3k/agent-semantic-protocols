pub struct UserSummary {
    pub user_id: u64,
    pub name: String,
    pub active: bool,
}

impl UserSummary {
    pub fn label(&self) -> String {
        if self.active {
            format!("{}#{}", self.name, self.user_id)
        } else {
            "inactive".to_string()
        }
    }
}
