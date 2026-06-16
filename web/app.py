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

app = FastAPI(title="Family Chat Archiver — Web")
app.mount("/static", StaticFiles(directory="static"), name="static")
templates = Jinja2Templates(directory="templates")

PAGE_SIZE = 50


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
):
    chat_id = _to_int(chat_id)
    user_id = _to_int(user_id)
    message_type = message_type or None
    offset = (page - 1) * PAGE_SIZE
    messages = db.list_messages(
        chat_id=chat_id, user_id=user_id, message_type=message_type,
        date_from=date_from or None, date_to=date_to or None,
        search=q or None, limit=PAGE_SIZE, offset=offset
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
        "page_size": PAGE_SIZE,
        "total_pages": (total + PAGE_SIZE - 1) // PAGE_SIZE,
        "chats": db.list_chats(),
        "message_types": db.list_message_types(),
        "filters": {
            "chat_id": chat_id, "user_id": user_id,
            "message_type": message_type,
            "date_from": date_from, "date_to": date_to, "q": q,
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
    affected = db.delete_message(message_id)
    if not affected:
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
    """Stream media file from Telegram (or local cache)."""
    media = db.get_media_by_id(media_id)
    if not media:
        raise HTTPException(404, "Media not found")

    file_id = media.get("file_id")
    file_unique_id = media.get("file_unique_id")
    if not file_id or not file_unique_id:
        raise HTTPException(404, "No file_id stored")

    # Use mime_type stored or guess from filename
    suggested_ext = ""
    if media.get("file_name"):
        import os
        _, suggested_ext = os.path.splitext(media["file_name"])

    path = await tg.fetch_media(file_id, file_unique_id, suggested_ext)
    if not path:
        raise HTTPException(502, "Failed to fetch media from Telegram")

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
