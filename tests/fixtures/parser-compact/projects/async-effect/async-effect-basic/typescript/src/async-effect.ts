export interface ResponseLike {
  readonly ok: boolean;
  text(): Promise<string>;
}

export interface Client {
  fetch(userId: string): Promise<ResponseLike>;
}

export async function loadUser(client: Client, userId: string): Promise<string> {
  const response = await client.fetch(userId);
  if (!response.ok) {
    throw new Error("missing");
  }
  const body = await response.text();
  return body;
}
