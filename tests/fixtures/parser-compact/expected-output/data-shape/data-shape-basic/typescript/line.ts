class UserSummary
  constructor
    field readonly userId: number
    field readonly name: string
    field readonly active: boolean
  method label
    if this.active
      return template
    return string
