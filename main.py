import ctypes
import logging
import sys
import threading

from PySide6.QtCore import QObject, Signal
from PySide6.QtWidgets import QApplication

import audio
import clipboard
import gpt_formatter
import history
import hotkeys
import settings
import whisper_service
from tray import Tray

ERROR_ALREADY_EXISTS = 183


class Bridge(QObject):
    state = Signal(str)
    history_updated = Signal()


class App:
    def __init__(self):
        self.busy = False
        self.smart = False
        self.bridge = Bridge()
        self.tray = Tray(self.restart_hotkeys, self.quit)
        self.bridge.state.connect(self.tray.set_state)
        self.bridge.history_updated.connect(self.tray.refresh_history)
        self.restart_hotkeys()
        threading.Thread(target=self.preload_model, daemon=True).start()

    def preload_model(self):
        try:
            whisper_service.load(settings.get("whisper_model"))
        except Exception:
            logging.exception("Не удалось загрузить модель Whisper")

    def restart_hotkeys(self):
        hotkeys.start(settings.get("hotkey"), self.on_press, self.on_release)

    def on_press(self, smart):
        if self.busy:
            return
        self.smart = smart
        try:
            audio.start()
        except Exception:
            logging.exception("Не удалось начать запись")
            return
        self.bridge.state.emit("recording")

    def on_release(self):
        if self.busy:
            return
        self.busy = True
        self.bridge.state.emit("processing")
        threading.Thread(target=self.process, daemon=True).start()

    def process(self):
        try:
            recording = audio.stop()
            if recording is None:
                return
            text = whisper_service.transcribe(
                recording, settings.get("whisper_model"), settings.get("language")
            )
            if not text:
                return
            if settings.get("use_gpt"):
                text = gpt_formatter.format_text(text, self.smart, settings.get("api_key"))
            history.add(text)
            self.bridge.history_updated.emit()
            clipboard.paste(text, settings.get("paste_mode"))
        except Exception:
            logging.exception("Ошибка обработки записи")
        finally:
            self.busy = False
            self.bridge.state.emit("idle")

    def quit(self):
        QApplication.quit()


def already_running():
    ctypes.windll.kernel32.CreateMutexW(None, False, "VoiceInput_SingleInstance")
    return ctypes.windll.kernel32.GetLastError() == ERROR_ALREADY_EXISTS


def main():
    if already_running():
        sys.exit(0)

    settings.setup_logging()
    settings.load()
    history.load()

    qt_app = QApplication(sys.argv)
    qt_app.setQuitOnLastWindowClosed(False)
    app = App()
    sys.exit(qt_app.exec())


if __name__ == "__main__":
    main()
