import ctypes
import logging
import time
from ctypes import wintypes
from pathlib import Path

import keyboard
import pyperclip

# в терминалах Ctrl+V занят самим терминалом, вставка идёт через Ctrl+Shift+V
TERMINALS = {
    "warp.exe",
    "windowsterminal.exe",
    "wt.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "conhost.exe",
    "wezterm-gui.exe",
    "alacritty.exe",
    "mintty.exe",
    "hyper.exe",
    "tabby.exe",
    "kitty.exe",
}

PROCESS_QUERY_LIMITED_INFORMATION = 0x1000

_user32 = ctypes.windll.user32
_kernel32 = ctypes.windll.kernel32


def _active_process():
    try:
        hwnd = _user32.GetForegroundWindow()
        pid = wintypes.DWORD()
        _user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        handle = _kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
        if not handle:
            return ""
        try:
            buffer = ctypes.create_unicode_buffer(512)
            size = wintypes.DWORD(512)
            if not _kernel32.QueryFullProcessImageNameW(handle, 0, buffer, ctypes.byref(size)):
                return ""
            return Path(buffer.value).name.lower()
        finally:
            _kernel32.CloseHandle(handle)
    except Exception:
        logging.exception("Не удалось определить активное окно")
        return ""


def paste(text, mode="auto"):
    for modifier in ("shift", "ctrl", "alt"):
        keyboard.release(modifier)
    time.sleep(0.05)

    if mode == "type":
        keyboard.write(text, delay=0)
        return

    if mode == "auto":
        mode = "ctrl+shift+v" if _active_process() in TERMINALS else "ctrl+v"

    try:
        old = pyperclip.paste()
    except Exception:
        old = ""

    pyperclip.copy(text)
    keyboard.send(mode)
    time.sleep(0.2)

    try:
        pyperclip.copy(old)
    except Exception:
        logging.exception("Не удалось восстановить буфер обмена")
