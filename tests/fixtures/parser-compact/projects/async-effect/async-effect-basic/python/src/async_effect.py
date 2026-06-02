"""Async effect compact fixture for parser snapshot tests."""


class Response:
    def __init__(self, ok: bool, body: str) -> None:
        self.ok = ok
        self.body = body

    async def text(self) -> str:
        return self.body


class Client:
    async def fetch(self, user_id: int) -> Response:
        if user_id <= 0:
            return Response(False, "")
        return Response(True, f"user:{user_id}")


async def load_user(client: Client, user_id: int) -> str:
    response = await client.fetch(user_id)
    if not response.ok:
        raise ValueError("missing")
    body = await response.text()
    return body
