from pathlib import Path

from PyInstaller.utils.hooks import collect_data_files, collect_dynamic_libs

# Whisper их не использует, а весят они 340 МБ и не дают архиву влезть в лимит GitHub
SKIP_DLLS = {"cudnn_adv64_9.dll", "nvrtc64_120_0.alt.dll", "nvblas64_12.dll"}

# DLL из pip-пакетов nvidia-* PyInstaller сам не находит
nvidia = Path(SPECPATH) / ".venv" / "Lib" / "site-packages" / "nvidia"
cuda_dlls = [(str(dll), ".") for dll in nvidia.glob("*/bin/*.dll") if dll.name not in SKIP_DLLS]

binaries = cuda_dlls + collect_dynamic_libs("ctranslate2") + collect_dynamic_libs("onnxruntime")
datas = collect_data_files("faster_whisper")

a = Analysis(
    ["main.py"],
    pathex=[],
    binaries=binaries,
    datas=datas,
    hiddenimports=["sounddevice", "pyperclip"],
    excludes=["tkinter", "matplotlib", "PIL", "scipy", "pandas"],
    noarchive=False,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    exclude_binaries=True,
    name="VoiceInput",
    console=False,
    upx=False,
)
coll = COLLECT(exe, a.binaries, a.datas, upx=False, name="VoiceInput")
