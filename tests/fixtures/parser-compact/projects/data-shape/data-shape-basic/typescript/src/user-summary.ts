export class UserSummary {
  constructor(
    public readonly userId: number,
    public readonly name: string,
    public readonly active: boolean,
  ) {}

  label(): string {
    if (this.active) {
      return `${this.name}#${this.userId}`;
    }
    return "inactive";
  }
}
