import logging
from fastapi import FastAPI, Request, Query, HTTPException
from fastapi.responses import HTMLResponse, FileResponse, RedirectResponse, JSONResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
import uvicorn

import config
import db
import telegram as tg

logging.basicConfig(level=logging.INFO,
                    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

app = FastAPI(title="Family Chat Archiver — Web", version=config.VERSION)
app.mount("/static", StaticFiles(directory="static"), name="static")
templates = Jinja2Templates(directory="templates")

# Make VERSION available to all templates
@app.middleware("http")
async def add_version_to_request(request, call_next):
    request.state.version = config.VERSION
    return await call_next(request)


templates.env.globals['VERSION'] = config.VERSION

DEFAULT_PAGE_SIZE = 100
ALLOWED_PAGE_SIZES = list(range(25, 1001, 25))  # 25, 50, 75, ..., 1000


def _to_int(v):
    """Convert empty string / None to None, else int."""
    if v is None or v == '':
        return None
    try:
        return int(v)
    except (ValueError, TypeError):
        return None


@app.get("/", response_class=HTMLResponse)
def index(
    request: Request,
    chat_id: str | None = Query(None),
    user_id: str | None = Query(None),
    message_type: str | None = Query(None),
    date_from: str | None = Query(None),
    date_to: str | None = Query(None),
    q: str | None = Query(None),
    page: int = Query(1, ge=1),
    page_size: int = Query(DEFAULT_PAGE_SIZE),
):
    chat_id = _to_int(chat_id)
    user_id = _to_int(user_id)
    message_type = message_type or None
    if page_size not in ALLOWED_PAGE_SIZES:
        page_size = DEFAULT_PAGE_SIZE
    offset = (page - 1) * page_size
    messages = db.list_messages(
        chat_id=chat_id, user_id=user_id, message_type=message_type,
        date_from=date_from or None, date_to=date_to or None,
        search=q or None, limit=page_size, offset=offset
    )
    total = db.count_messages(
        chat_id=chat_id, user_id=user_id, message_type=message_type,
        date_from=date_from or None, date_to=date_to or None, search=q or None
    )

    return templates.TemplateResponse("index.html", {
        "request": request,
        "messages": messages,
        "total": total,
        "page": page,
        "page_size": page_size,
        "page_size_options": ALLOWED_PAGE_SIZES,
        "total_pages": (total + page_size - 1) // page_size,
        "chats": db.list_chats(),
        "message_types": db.list_message_types(),
        "filters": {
            "chat_id": chat_id, "user_id": user_id,
            "message_type": message_type,
            "date_from": date_from, "date_to": date_to, "q": q,
            "page_size": page_size if page_size != DEFAULT_PAGE_SIZE else None,
        },
    })


@app.get("/message/{message_id}", response_class=HTMLResponse)
def view_message(request: Request, message_id: int):
    msg = db.get_message(message_id)
    if not msg:
        raise HTTPException(404, "Message not found")
    return templates.TemplateResponse("message.html", {
        "request": request, "msg": msg,
    })


@app.post("/message/{message_id}/delete")
def delete_message(message_id: int):
    if not db.soft_delete_message(message_id):
        raise HTTPException(404, "Message not found or already deleted")
    return RedirectResponse(f"/message/{message_id}", status_code=303)


@app.post("/message/{message_id}/restore")
def restore_message(message_id: int):
    if not db.restore_message(message_id):
        raise HTTPException(404, "Message not found")
    return RedirectResponse(f"/message/{message_id}", status_code=303)


@app.post("/message/{message_id}/hard-delete")
def hard_delete_message(message_id: int):
    if not db.hard_delete_message(message_id):
        raise HTTPException(404, "Message not found")
    return RedirectResponse("/", status_code=303)


@app.get("/users", response_class=HTMLResponse)
def users(request: Request):
    return templates.TemplateResponse("users.html", {
        "request": request,
        "users": db.list_users(),
    })


@app.get("/stats", response_class=HTMLResponse)
def stats(request: Request):
    return templates.TemplateResponse("stats.html", {
        "request": request,
        "overview": db.stats_overview(),
        "messages_per_day": db.stats_messages_per_day(30),
        "top_users": db.top_users(10),
        "message_types": db.list_message_types(),
    })


@app.get("/corrections", response_class=HTMLResponse)
def corrections(request: Request, page: int = Query(1, ge=1)):
    offset = (page - 1) * PAGE_SIZE
    return templates.TemplateResponse("corrections.html", {
        "request": request,
        "corrections": db.list_corrections(limit=PAGE_SIZE, offset=offset),
        "page": page,
    })


@app.get("/media/{media_id}")
async def media_file(media_id: int):
    """Serve media. Priority: bot's local_path → web cache → fetch from Telegram."""
    import os
    media = db.get_media_by_id(media_id)
    if not media:
        raise HTTPException(404, "Media not found")

    # 1. Bot-saved local file (preferred, no Telegram needed)
    local_path = media.get("local_path")
    if local_path and os.path.exists(local_path):
        mime = media.get("mime_type") or tg.mime_from_ext(local_path)
        return FileResponse(local_path, media_type=mime)

    # 2. Fallback: download from Telegram (and cache in web/media_cache)
    file_id = media.get("file_id")
    file_unique_id = media.get("file_unique_id")
    if not file_id or not file_unique_id:
        raise HTTPException(404, "No file stored or referenced")

    suggested_ext = ""
    if media.get("file_name"):
        _, suggested_ext = os.path.splitext(media["file_name"])

    path = await tg.fetch_media(file_id, file_unique_id, suggested_ext)
    if not path:
        raise HTTPException(502, "Failed to fetch media from Telegram (file may have expired)")

    mime = media.get("mime_type") or tg.mime_from_ext(path)
    return FileResponse(path, media_type=mime)


@app.get("/api/stats/per-day")
def api_stats_per_day(days: int = 30):
    return JSONResponse([
        {"day": str(r['day']), "count": r['c']}
        for r in db.stats_messages_per_day(days)
    ])


if __name__ == "__main__":
    uvicorn.run(app, host=config.WEB_HOST, port=config.WEB_PORT)
