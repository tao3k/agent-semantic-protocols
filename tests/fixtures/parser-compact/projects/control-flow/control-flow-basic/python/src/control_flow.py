"""Control-flow fixture for parser compact snapshots."""


def decide(flag: bool, values: list[int]) -> int | None:
    total = 0
    for value in values:
        if value < 0:
            return None
        total += value
    if flag and total > 10:
        return total
    return 0
