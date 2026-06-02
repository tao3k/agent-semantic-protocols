export class UserSummary
  constructor
  label(): string
    if this.active
      return `${this.name}#${this.userId}`
    return "inactive"
