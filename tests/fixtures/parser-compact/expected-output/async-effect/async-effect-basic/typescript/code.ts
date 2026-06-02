export async function loadUser(client: Client, userId: string): Promise<string>
  const response = await client.fetch(userId)
    await
      fetch
  if !response.ok
    throw new Error("missing")
  const body = await response.text()
      text
  return body
