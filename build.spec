from pathlib import Path

from PyInstaller.utils.hooks import collect_data_files, collect_dynamic_libs

# DLL из pip-пакетов nvidia-* PyInstaller сам не находит
nvidia = Path(SPECPATH) / ".venv" / "Lib" / "site-packages" / "nvidia"
cuda_dlls = [(str(dll), ".") for dll in nvidia.glob("*/bin/*.dll")]

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
