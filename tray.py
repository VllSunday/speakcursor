import keyboard
import pyperclip
from PySide6.QtCore import Qt, QTimer
from PySide6.QtGui import QAction, QColor, QIcon, QPainter, QPixmap
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QLineEdit,
    QMenu,
    QMessageBox,
    QSystemTrayIcon,
)

import history
import settings

STATE_COLORS = {
    "idle": "#9aa0a6",
    "recording": "#e53935",
    "processing": "#fb8c00",
}
STATE_TEXTS = {
    "idle": "Голосовой ввод — удерживайте {hotkey}",
    "recording": "Запись...",
    "processing": "Обработка...",
}

HOTKEYS = ["f8", "right ctrl"]
MODELS = ["large-v3", "medium", "small", "base", "tiny"]
LANGUAGES = {"Авто": "auto", "Русский": "ru"}
PASTE_MODES = {
    "Авто (терминалы — Ctrl+Shift+V)": "auto",
    "Всегда Ctrl+V": "ctrl+v",
    "Всегда Ctrl+Shift+V": "ctrl+shift+v",
    "Печатать текст (работает везде)": "type",
}

PREVIEW_LENGTH = 50


def _icon(state):
    pixmap = QPixmap(64, 64)
    pixmap.fill(Qt.transparent)
    painter = QPainter(pixmap)
    painter.setRenderHint(QPainter.Antialiasing)
    painter.setBrush(QColor(STATE_COLORS[state]))
    painter.setPen(Qt.NoPen)
    painter.drawRoundedRect(22, 8, 20, 32, 10, 10)
    painter.drawRect(30, 44, 4, 10)
    painter.drawRect(20, 52, 24, 4)
    painter.end()
    return QIcon(pixmap)


class SettingsDialog(QDialog):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Настройки — Голосовой ввод")
        self.setMinimumWidth(380)

        self.api_key = QLineEdit(settings.get("api_key"))
        self.api_key.setEchoMode(QLineEdit.Password)
        self.api_key.setPlaceholderText("sk-...")

        self.hotkey = QComboBox()
        self.hotkey.addItems(HOTKEYS)
        self.hotkey.setCurrentText(settings.get("hotkey"))

        self.language = QComboBox()
        self.language.addItems(LANGUAGES.keys())
        for label, code in LANGUAGES.items():
            if code == settings.get("language"):
                self.language.setCurrentText(label)

        self.model = QComboBox()
        self.model.addItems(MODELS)
        self.model.setCurrentText(settings.get("whisper_model"))

        self.paste_mode = QComboBox()
        self.paste_mode.addItems(PASTE_MODES.keys())
        for label, code in PASTE_MODES.items():
            if code == settings.get("paste_mode"):
                self.paste_mode.setCurrentText(label)

        self.use_gpt = QCheckBox("Обрабатывать текст через GPT")
        self.use_gpt.setChecked(settings.get("use_gpt"))

        self.autostart = QCheckBox("Запускать вместе с Windows")
        self.autostart.setChecked(settings.get("autostart"))

        buttons = QDialogButtonBox(QDialogButtonBox.Ok | QDialogButtonBox.Cancel)
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        layout = QFormLayout(self)
        layout.addRow("OpenAI API Key:", self.api_key)
        layout.addRow("Горячая клавиша:", self.hotkey)
        layout.addRow("Язык:", self.language)
        layout.addRow("Модель Whisper:", self.model)
        layout.addRow("Вставка:", self.paste_mode)
        layout.addRow(self.use_gpt)
        layout.addRow(self.autostart)
        layout.addRow(buttons)

    def apply(self):
        settings.set("api_key", self.api_key.text().strip())
        settings.set("hotkey", self.hotkey.currentText())
        settings.set("language", LANGUAGES[self.language.currentText()])
        settings.set("whisper_model", self.model.currentText())
        settings.set("paste_mode", PASTE_MODES[self.paste_mode.currentText()])
        settings.set("use_gpt", self.use_gpt.isChecked())
        settings.set("autostart", self.autostart.isChecked())
        settings.save()


class Tray(QSystemTrayIcon):
    def __init__(self, on_settings_changed, on_quit):
        super().__init__()
        self.on_settings_changed = on_settings_changed

        self.menu = QMenu()
        self.history_menu = QMenu("История")
        self.menu.addMenu(self.history_menu)
        self.menu.addAction(QAction("Настройки", self, triggered=self.open_settings))
        self.menu.addAction(QAction("О программе", self, triggered=self.show_about))
        self.menu.addSeparator()
        self.menu.addAction(QAction("Выход", self, triggered=on_quit))
        self.setContextMenu(self.menu)

        self.refresh_history()
        self.set_state("idle")
        self.show()

    def set_state(self, state):
        self.setIcon(_icon(state))
        self.setToolTip(STATE_TEXTS[state].format(hotkey=settings.get("hotkey").upper()))

    def refresh_history(self):
        self.history_menu.clear()
        items = history.items()
        if not items:
            empty = QAction("Пусто", self)
            empty.setEnabled(False)
            self.history_menu.addAction(empty)
            return
        for item in items:
            preview = item["text"].replace("\n", " ")
            if len(preview) > PREVIEW_LENGTH:
                preview = preview[:PREVIEW_LENGTH] + "…"
            action = QAction(f'{item["time"]}  {preview}', self)
            action.triggered.connect(lambda _, text=item["text"]: self._paste_from_history(text))
            self.history_menu.addAction(action)

    def _paste_from_history(self, text):
        pyperclip.copy(text)
        # ждём, пока Windows вернёт фокус окну, которое было активным до открытия меню
        QTimer.singleShot(400, lambda: keyboard.send("ctrl+v"))

    def open_settings(self):
        dialog = SettingsDialog()
        if dialog.exec() == QDialog.Accepted:
            dialog.apply()
            self.on_settings_changed()
            self.set_state("idle")

    def show_about(self):
        hotkey = settings.get("hotkey").upper()
        QMessageBox.information(
            None,
            "О программе",
            "Голосовой ввод\n\n"
            f"Удерживайте {hotkey} — обычная диктовка.\n"
            f"Shift + {hotkey} — умная диктовка, текст структурируется.\n\n"
            "Распознавание: faster-whisper, локально.\n"
            "Постобработка: OpenAI, по желанию.",
        )
