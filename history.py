import json
import logging
from datetime import datetime

from settings import APP_DIR

HISTORY_FILE = APP_DIR / "history.json"
MAX_ITEMS = 20

_items = []


def load():
    global _items
    if HISTORY_FILE.exists():
        try:
            _items = json.loads(HISTORY_FILE.read_text(encoding="utf-8-sig"))
        except Exception:
            logging.exception("Не удалось прочитать history.json")
            _items = []
    return _items


def add(text):
    _items.insert(0, {"time": datetime.now().strftime("%H:%M"), "text": text})
    del _items[MAX_ITEMS:]
    try:
        APP_DIR.mkdir(parents=True, exist_ok=True)
        HISTORY_FILE.write_text(json.dumps(_items, ensure_ascii=False, indent=2), encoding="utf-8")
    except Exception:
        logging.exception("Не удалось сохранить историю")


def items():
    return _items
