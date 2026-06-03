def decide
  assign total
  for value in values
    if value < 0
      return None
    assign total += value
  if flag and total > 10
    return total
  return 0
