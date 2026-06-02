def parse_score(text: str) -> int:
  try:
    value = int(text)
  except ValueError:
    raise ValueError('invalid')
  if value < 0:
    raise ValueError('negative')
  return value
