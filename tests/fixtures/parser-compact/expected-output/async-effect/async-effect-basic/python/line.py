async def load_user
  assign response
    await fetch
  if not response.ok
    raise ValueError:missing
  assign body
    await text
  return body
