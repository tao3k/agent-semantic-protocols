class UserSummary:
  user_id: int
  name: str
  active: bool
  def label(self) -> str:
    if self.active:
      return f'{self.name}#{self.user_id}'
    return 'inactive'
