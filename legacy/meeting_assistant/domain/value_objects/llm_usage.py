from dataclasses import dataclass


@dataclass(frozen=True)
class LLMUsage:
    input_tokens: int = 0
    output_tokens: int = 0
    cached_tokens: int = 0
    cost_usd: float = 0.0
