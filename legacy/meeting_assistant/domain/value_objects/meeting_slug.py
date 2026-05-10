import re
from dataclasses import dataclass

_VALID = re.compile(r"^[\w\-]+$")
_UNSAFE = re.compile(r"[^\w\-]")


@dataclass(frozen=True)
class MeetingSlug:
    value: str

    def __post_init__(self):
        if not _VALID.match(self.value):
            raise ValueError(f"Invalid meeting slug: {self.value!r}")

    @classmethod
    def from_title(cls, title: str, date_prefix: str = "") -> "MeetingSlug":
        safe = _UNSAFE.sub("_", title)
        return cls(f"{date_prefix}_{safe}" if date_prefix else safe)

    def __str__(self) -> str:
        return self.value
