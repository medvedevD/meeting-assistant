from fastapi import APIRouter, Request, HTTPException
from ..schemas import TemplateSaveRequest, TemplateDeleteRequest

router = APIRouter()


@router.get("/templates")
def get_templates(request: Request):
    container = request.app.state.container
    cfg = container.config.load()
    return {
        "templates": container.template_repo.list(),
        "active": cfg.get("protocol", {}).get("active_template", ""),
    }


@router.post("/template/save")
def post_template_save(data: TemplateSaveRequest, request: Request):
    if not data.name.strip():
        raise HTTPException(400, "name required")
    container = request.app.state.container
    container.template_repo.save(data.name, data.prompt)
    if data.active:
        cfg = container.config.load()
        cfg["protocol"]["active_template"] = data.active
        container.config.save(cfg)
    return {"ok": True}


@router.post("/template/delete")
def post_template_delete(data: TemplateDeleteRequest, request: Request):
    if not data.name.strip():
        raise HTTPException(400, "name required")
    request.app.state.container.template_repo.delete(data.name)
    return {"ok": True}
