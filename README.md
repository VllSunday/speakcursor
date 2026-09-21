<div align="center">

# SpeakCursor

### Offline voice typing and push-to-talk dictation for Windows

`Windows 10/11` &nbsp;·&nbsp; `Python` &nbsp;·&nbsp; `Whisper / faster-whisper` &nbsp;·&nbsp; `System tray`

[Quick start](#quick-start) · [Features](#features) · [Smart dictation](#smart-dictation) · [Privacy](#privacy) · [Build an exe](#build-an-exe)

</div>

<p align="center">
  <img src="assets/demo.gif" alt="Hold F8, speak, release — text appears at the cursor" width="820">
</p>

> **Speak, release, keep working.** SpeakCursor is a Windows voice typing app that turns speech into text at the active cursor. Local Whisper speech-to-text works without an internet connection; GPT cleanup is optional.

[Русская версия](README.ru.md)

## Features

| Feature                          | What it does                                                                         |
| -------------------------------- | ------------------------------------------------------------------------------------ |
| **F8 — dictation**               | Hold the key, speak, release: recognize, optionally clean up, and paste              |
| **Shift + F8 — smart dictation** | Turn a stream of thought into structured Markdown for specs, notes, or README drafts |
| **Local by default**             | Whisper transcription stays on the computer; no internet is required                 |
| **Works where you type**         | Pastes into apps and terminals, then restores the previous clipboard contents        |
| **Lives in the tray**            | Recording and processing state, settings, startup, and the last 20 transcripts       |

The app has no main window — only a tray icon and a settings dialog.

## Smart dictation

The feature that sets this apart from other Whisper dictation tools. Hold Shift+F8, think out
loud, and the rambling gets turned into a clean document instead of a wall of text.

You say:

> so basically we need a settings screen where the user can put in the api key and also pick a
> model and uh also there should be a hotkey field and it should save automatically and yeah
> the autostart checkbox too

You get:

```markdown
## Settings screen

- API key input
- Model selection
- Hotkey field
- Autostart checkbox

Settings are saved automatically.
```

## Requirements

- Windows 10/11, Python 3.12
- A microphone
- NVIDIA GPU — recommended, but not required

On a GPU (tested on an RTX 5070 Ti) the `large-v3` model transcribes 11 seconds of speech in
about one second. Without a GPU the app falls back to CPU, where `large-v3` is noticeably slow —
pick a smaller model (`small` or `base`) in the settings.

## Quick start

```powershell
git clone https://github.com/VllSunday/speakcursor.git
cd speakcursor
python -m venv .venv
.venv\Scripts\pip install -r requirements.txt
.venv\Scripts\pythonw.exe main.py
```

The first run downloads the Whisper model (`large-v3` is about 3 GB) into the HuggingFace cache.
The tray icon shows up immediately, but recognition only starts working after ~15 seconds.

## Privacy

Without an API key — or with the **Use GPT** checkbox off — the app pastes raw Whisper text and no transcription leaves the computer.

## Optional GPT cleanup

Raw Whisper output can mangle technical terms — it happily turns "Whisper" into something else
entirely. Sending the transcript through OpenAI fixes terms, grammar and punctuation.

Paste an API key into Settings, or set the `OPENAI_API_KEY` environment variable. The model is
configurable (`gpt-5-nano`, `gpt-5-mini`, `gpt-4.1-mini`, `gpt-4o-mini`, or anything you type in).
Cost per dictation is negligible.

Without a key — or with the "Use GPT" checkbox off — the raw Whisper text is pasted and nothing
ever leaves your machine.

## Administrator rights

If you dictate into a window that runs as administrator (an elevated terminal, Registry Editor,
Task Manager), the app has to run as administrator too. Windows blocks input sent from a normal
process into an elevated window, so the text simply will not paste.

The "Start with Windows" checkbox creates a Scheduled Task with highest privileges: it starts
silently at logon with no UAC prompt. To create it, enable the checkbox while running the app as
administrator.

If you never dictate into elevated windows, a normal launch is fine.

## Pasting

Default mode is "Auto": terminals (Warp, Windows Terminal, cmd, PowerShell, WezTerm, Alacritty…)
get Ctrl+Shift+V, because Ctrl+V is taken by the terminal itself; everything else gets Ctrl+V.
The clipboard contents are saved and restored afterwards.

If some app understands neither shortcut, switch to "Type text" mode in the settings — it emulates
keystrokes, works everywhere and never touches the clipboard, but is slower for long text.

## Freeing memory when idle

The `large-v3` model holds about 3 GB of VRAM the whole time the app sits in the tray, which is
annoying when the GPU is needed for something else. "Unload model from memory" in the settings
frees it after 5, 10 or 15 minutes without dictation; "Never" (the default) keeps it loaded.

The trade-off: the first dictation after an unload takes a few seconds longer, because the model
is read from disk again. Everything after that is as fast as usual.

## Build an .exe

```powershell
.venv\Scripts\pip install pyinstaller
.venv\Scripts\pyinstaller build.spec
```

`dist\VoiceInput\VoiceInput.exe` runs without a console window. The build is about 3 GB, almost
entirely CUDA libraries (cuDNN and cuBLAS).

## Data location

`%APPDATA%\VoiceInput\` holds settings (`settings.json`), history (`history.json`) and the error
log (`error.log`).

## Layout

| File                 | Purpose                                |
| -------------------- | -------------------------------------- |
| `main.py`            | entry point, wires everything together |
| `tray.py`            | tray icon, menu, settings dialog       |
| `hotkeys.py`         | hotkey hold detection                  |
| `audio.py`           | microphone recording                   |
| `whisper_service.py` | local recognition (faster-whisper)     |
| `gpt_formatter.py`   | optional OpenAI cleanup                |
| `clipboard.py`       | pasting into the active window         |
| `settings.py`        | settings and autostart                 |
| `history.py`         | last 20 transcripts                    |
