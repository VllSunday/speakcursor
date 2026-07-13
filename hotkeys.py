import keyboard

_on_press = None
_on_release = None
_held = False


def start(hotkey, on_press, on_release):
    global _on_press, _on_release, _held
    _on_press, _on_release, _held = on_press, on_release, False
    keyboard.unhook_all()
    keyboard.on_press_key(hotkey, _handle_press)
    keyboard.on_release_key(hotkey, _handle_release)


def _handle_press(event):
    global _held
    if _held:
        return
    _held = True
    _on_press(keyboard.is_pressed("shift"))


def _handle_release(event):
    global _held
    if not _held:
        return
    _held = False
    _on_release()
