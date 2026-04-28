import re
from pathlib import Path
from ...domain.ports.template_repository import ITemplateRepository

_UNSAFE = re.compile(r"[/\\\0]")


class FilesystemTemplateRepository(ITemplateRepository):
    def __init__(self, prompts_dir: Path):
        self._dir = prompts_dir

    def list(self) -> list[dict]:
        self._dir.mkdir(exist_ok=True)
        return [
            {"name": f.stem, "prompt": f.read_text(encoding="utf-8")}
            for f in sorted(self._dir.glob("*.md"))
        ]

    def save(self, name: str, prompt: str) -> None:
        self._dir.mkdir(exist_ok=True)
        self._path(name).write_text(prompt, encoding="utf-8")

    def delete(self, name: str) -> None:
        p = self._path(name)
        if p.exists():
            p.unlink()

    def _path(self, name: str) -> Path:
        return self._dir / f"{_UNSAFE.sub('_', name)}.md"
