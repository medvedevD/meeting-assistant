from ..domain.ports.template_repository import ITemplateRepository


class ManageTemplatesUseCase:
    def __init__(self, template_repo: ITemplateRepository):
        self._repo = template_repo

    def list(self) -> list[dict]:
        return self._repo.list()

    def save(self, name: str, prompt: str) -> None:
        self._repo.save(name, prompt)

    def delete(self, name: str) -> None:
        self._repo.delete(name)
