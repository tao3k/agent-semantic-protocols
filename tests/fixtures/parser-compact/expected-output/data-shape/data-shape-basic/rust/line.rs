pub struct UserSummary
field pub user_id: u64
field pub name: String
field pub active: bool
impl UserSummary
fn label
if self.active
return format!
else
return to_string
