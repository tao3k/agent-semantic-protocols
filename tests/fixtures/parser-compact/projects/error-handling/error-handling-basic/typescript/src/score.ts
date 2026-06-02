export function parseScore(text: string): number {
  const value = Number.parseInt(text, 10);
  if (Number.isNaN(value)) {
    throw new Error("invalid");
  }
  if (value < 0) {
    throw new Error("negative");
  }
  return value;
}
