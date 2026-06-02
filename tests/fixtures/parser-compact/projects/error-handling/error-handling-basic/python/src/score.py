"""Error handling compact fixture for parser snapshot tests."""


def parse_score(text: str) -> int:
    try:
        value = int(text)
    except ValueError as error:
        raise ValueError("invalid") from error
    if value < 0:
        raise ValueError("negative")
    return value
