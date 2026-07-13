import json
import logging
import os
import subprocess
import sys
from pathlib import Path

APP_NAME = "VoiceInput"
APP_DIR = Path(os.environ["APPDATA"]) / APP_NAME
SETTINGS_FILE = APP_DIR / "settings.json"
LOG_FILE = APP_DIR / "error.log"

DEFAULTS = {
    "api_key": "",
    "hotkey": "f8",
    "language": "auto",
    "use_gpt": True,
    "autostart": False,
    "whisper_model": "large-v3",
    "gpt_model": "gpt-5-nano",
    "paste_mode": "auto",
    "unload_after": 0,
}

_settings = dict(DEFAULTS)


def load():
    APP_DIR.mkdir(parents=True, exist_ok=True)
    if SETTINGS_FILE.exists():
        try:
            _settings.update(json.loads(SETTINGS_FILE.read_text(encoding="utf-8-sig")))
        except Exception:
            logging.exception("Не удалось прочитать settings.json, беру значения по умолчанию")
    return _settings


def save():
    APP_DIR.mkdir(parents=True, exist_ok=True)
    SETTINGS_FILE.write_text(json.dumps(_settings, ensure_ascii=False, indent=2), encoding="utf-8")
    set_autostart(_settings["autostart"])


def get(key):
    if key == "api_key" and not _settings.get("api_key"):
        return os.environ.get("OPENAI_API_KEY", "")
    return _settings.get(key, DEFAULTS.get(key))


def set(key, value):
    _settings[key] = value


def setup_logging():
    APP_DIR.mkdir(parents=True, exist_ok=True)
    logging.basicConfig(
        filename=LOG_FILE,
        level=logging.ERROR,
        format="%(asctime)s %(levelname)s %(message)s",
        encoding="utf-8",
    )


def _run_command():
    if getattr(sys, "frozen", False):
        return f'"{sys.executable}"'
    pythonw = Path(sys.executable).with_name("pythonw.exe")
    exe = pythonw if pythonw.exists() else Path(sys.executable)
    return f'"{exe}" "{Path(__file__).with_name("main.py")}"'


def set_autostart(enabled):
    # задача в планировщике, а не реестр Run: с правами администратора вставка
    # работает и в окнах, запущенных от администратора
    if enabled:
        command = [
            "schtasks", "/create", "/tn", APP_NAME, "/tr", _run_command(),
            "/sc", "onlogon", "/rl", "highest", "/f",
        ]
    else:
        command = ["schtasks", "/delete", "/tn", APP_NAME, "/f"]
    try:
        result = subprocess.run(
            command, capture_output=True, text=True, creationflags=subprocess.CREATE_NO_WINDOW
        )
        if result.returncode != 0 and enabled:
            logging.error("Не удалось включить автозапуск: %s", result.stderr.strip())
    except Exception:
        logging.exception("Не удалось изменить автозапуск")
