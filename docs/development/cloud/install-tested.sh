#!/usr/bin/env bash
set -euo pipefail
cd /workspace/speakcursor
setup_dir=/workspace/speakcursor-env
mkdir -p "$setup_dir"/{apt/lists/partial,apt/archives/partial,debs,cache,appdata}
export PIP_CACHE_DIR="$setup_dir/pip-cache"
python3.12 -m venv .venv
python3.12 - <<'PY'
from pathlib import Path
source = Path("requirements.txt").read_text()
lines = [line for line in source.splitlines() if not line.strip().startswith(("nvidia-cublas-cu12", "nvidia-cudnn-cu12"))]
Path("/workspace/speakcursor-env/requirements-cpu.txt").write_text("\n".join(lines) + "\n")
PY
cat > "$setup_dir/constraints.txt" <<'EOF'
# PyAV 19 removed metadata_errors used by faster-whisper 1.2.1.
faster-whisper==1.2.1
av==16.1.0
EOF
.venv/bin/python -m pip install -c "$setup_dir/constraints.txt" -r "$setup_dir/requirements-cpu.txt"
if [ ! -f "$setup_dir/sysroot/usr/lib/x86_64-linux-gnu/libportaudio.so.2" ]; then
  printf '%s\n' 'deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian trixie main' > "$setup_dir/apt/sources.list"
  apt_options=(-o Dir::Etc::sourcelist="$setup_dir/apt/sources.list" -o Dir::Etc::sourceparts=- -o Dir::State::lists="$setup_dir/apt/lists" -o Dir::Cache::archives="$setup_dir/apt/archives" -o APT::Get::List-Cleanup=0)
  /usr/bin/apt-get "${apt_options[@]}" update
  (cd "$setup_dir/debs" && /usr/bin/apt-get "${apt_options[@]}" download libportaudio2)
  for package in "$setup_dir"/debs/libportaudio2_*.deb; do
    dpkg-deb -x "$package" "$setup_dir/sysroot"
  done
fi
ln -sfn libportaudio.so.2 "$setup_dir/sysroot/usr/lib/x86_64-linux-gnu/libportaudio.so"
cat > "$setup_dir/env.sh" <<'EOF'
export PATH="/workspace/speakcursor/.venv/bin:$PATH"
export LD_LIBRARY_PATH="/workspace/speakcursor-env/sysroot/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export QT_QPA_PLATFORM=offscreen
export XDG_CACHE_HOME=/workspace/speakcursor-env/cache
export HF_HOME=/workspace/speakcursor-env/huggingface
export PIP_CACHE_DIR=/workspace/speakcursor-env/pip-cache
export APPDATA=/workspace/speakcursor-env/appdata
EOF
if [ ! -f "$setup_dir/jfk.flac" ]; then
  curl --fail --show-error --location https://raw.githubusercontent.com/openai/whisper/main/tests/jfk.flac -o "$setup_dir/jfk.flac"
fi
printf '%s\n' '63a4b1e4c1dc655ac70961ffbf518acd249df237e5a0152faae9a4a836949715  /workspace/speakcursor-env/jfk.flac' | sha256sum -c -
cat > "$setup_dir/smoke.py" <<'PY'
import json
import tempfile
from pathlib import Path
import numpy as np
import audio, settings, history, gpt_formatter, whisper_service
from PySide6.QtWidgets import QApplication
from faster_whisper.audio import decode_audio
from faster_whisper.vad import get_speech_timestamps
import tray

app = QApplication([])
dialog = tray.SettingsDialog()
assert dialog.model.currentText() == settings.get("whisper_model")
for state in tray.STATE_COLORS:
    assert not tray._icon(state).isNull()
print("PASS: settings dialog and three tray icons rendered offscreen")
demo = decode_audio("assets/demo.mp4", sampling_rate=16000)
assert 16 < len(demo) / 16000 < 17
speech = decode_audio("/workspace/speakcursor-env/jfk.flac", sampling_rate=16000)
assert get_speech_timestamps(speech)
assert not get_speech_timestamps(np.zeros(16000, dtype=np.float32))
print("PASS: demo decoding; ONNX speech and silence detection")
with tempfile.TemporaryDirectory(prefix="speakcursor-smoke-") as tmp:
    history.HISTORY_FILE = Path(tmp) / "history.json"
    history._items = []
    for i in range(25):
        history.add(f"Проверка {i}")
    saved = json.loads(history.HISTORY_FILE.read_text())
    assert len(saved) == 20 and saved[0]["text"] == "Проверка 24"
    history._items = []
    assert history.load() == saved
assert gpt_formatter.format_text("Проверка", False, "") == "Проверка"
assert audio.stop() is None
assert audio.sd.get_portaudio_version()[0] > 0
assert not whisper_service.is_loaded()
print("PASS: history persistence and limit, no-key GPT fallback, PortAudio import")
print("NOT TESTED: Whisper transcription (model download blocked), Windows desktop integration, physical microphone, CUDA, live OpenAI")
PY
source "$setup_dir/env.sh"
python -m pip check
python -c 'exec(compile(open("/workspace/speakcursor-env/smoke.py").read(), "/workspace/speakcursor-env/smoke.py", "exec"))'
