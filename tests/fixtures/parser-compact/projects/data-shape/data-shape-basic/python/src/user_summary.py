"""Data shape compact fixture for parser snapshot tests."""

from dataclasses import dataclass


@dataclass(frozen=True)
class UserSummary:
    user_id: int
    name: str
    active: bool

    def label(self) -> str:
        if self.active:
            return f"{self.name}#{self.user_id}"
        return "inactive"
