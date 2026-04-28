from fastapi import APIRouter, Request, Query

router = APIRouter()


@router.get("/search")
async def search_meetings(request: Request, q: str = Query(default="", min_length=1)):
    index = request.app.state.container.search_index
    results = index.search(q)
    return [
        {
            "slug": r.slug,
            "title": r.title,
            "snippet": r.snippet,
            "has_transcript": r.has_transcript,
            "has_protocol": r.has_protocol,
        }
        for r in results
    ]
