async def load_user(client: Client, user_id: int) -> str:
  response = await client.fetch(user_id)
  if not response.ok:
    raise ValueError('missing')
  body = await response.text()
  return body
