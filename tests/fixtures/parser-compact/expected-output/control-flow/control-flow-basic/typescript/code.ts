export function decide(flag: boolean, values: number[]): number | null
  let total = 0
  for const value of values
    if value < 0
      return null
    total += value
  if flag && total > 10
    return total
  return 0
