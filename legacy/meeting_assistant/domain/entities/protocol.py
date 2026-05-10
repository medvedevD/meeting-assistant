from dataclasses import dataclass
from datetime import datetime


@dataclass
class Protocol:
    content: str
    template_name: str
    generated_at: datetime
    model: str
