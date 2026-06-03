class UserSummary
  field user_id: int
  field name: str
  field active: bool
  def label
    if self.active
      return template
    return string
