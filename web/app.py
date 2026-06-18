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
# Defence-in-depth: force HTML autoescape so user-controlled content
# (message text, user names, chat titles) cannot inject script tags.
templates.env.autoescape = True

# Make VERSION available to all templates
@app.middleware("http")
async def add_version_to_request(request, call_next):
    request.state.version = config.VERSION
    return await call_next(request)


@app.middleware("http")
async def same_origin_for_unsafe_methods(request: Request, call_next):
    """Same-origin guard for state-changing requests (CSRF defence).

    The UI has no auth yet, but if it's ever put behind one, browser-issued
    POSTs from another origin must be rejected. Allowed Origin/Referer hosts
    are: the Host header itself, plus optional ALLOWED_ORIGINS env list.
    """
    if request.method in ("POST", "PUT", "PATCH", "DELETE"):
        host = request.headers.get("host", "")
        origin = request.headers.get("origin") or request.headers.get("referer") or ""
        if origin:
            from urllib.parse import urlparse
            netloc = urlparse(origin).netloc
            allowed = {host, *config.ALLOWED_ORIGINS}
            if netloc not in allowed:
                return JSONResponse({"detail": "Cross-origin request blocked"},
                                    status_code=403)
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
    q: str | None = Query(None, max_length=200),
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
    offset = (page - 1) * DEFAULT_PAGE_SIZE
    return templates.TemplateResponse("corrections.html", {
        "request": request,
        "corrections": db.list_corrections(limit=DEFAULT_PAGE_SIZE, offset=offset),
        "page": page,
    })


@app.get("/media/{media_id}")
async def media_file(media_id: int):
    """Serve media. Priority: bot's local_path → web cache → fetch from Telegram."""
    import os
    media = db.get_media_by_id(media_id)
    if not media:
        raise HTTPException(404, "Media not found")

    # 1. Bot-saved local file (preferred, no Telegram needed).
    # Validate the path is inside an allowed directory — defends against
    # path traversal if media.local_path is ever tampered with in the DB.
    local_path = media.get("local_path")
    if local_path:
        abs_local = os.path.abspath(local_path)
        if not any(
            abs_local == d or abs_local.startswith(d + os.sep)
            for d in config.ALLOWED_MEDIA_DIRS
        ):
            logger.warning("Refusing to serve media %s: path %s outside ALLOWED_MEDIA_DIRS",
                           media_id, abs_local)
            raise HTTPException(403, "Media path not allowed")
        if os.path.exists(abs_local):
            mime = media.get("mime_type") or tg.mime_from_ext(abs_local)
            return FileResponse(abs_local, media_type=mime)

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
    logger.info("Family Chat Archiver — Web v%s", config.VERSION)
    logger.info("Binding to http://%s:%s", config.WEB_HOST, config.WEB_PORT)
    # The UI has no built-in auth. Warn loudly if it's exposed beyond loopback.
    if config.WEB_HOST not in ("127.0.0.1", "localhost", "::1"):
        logger.warning(
            "WEB_HOST=%s is not loopback — the interface has NO authentication. "
            "Put it behind a reverse-proxy (nginx) with auth + HTTPS, or restrict "
            "access at the firewall level.", config.WEB_HOST
        )
    uvicorn.run(app, host=config.WEB_HOST, port=config.WEB_PORT)
