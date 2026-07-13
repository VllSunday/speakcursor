import gc
import logging
import os
import sys
import threading
from pathlib import Path

NO_SPEECH_THRESHOLD = 0.6

_model = None
_model_name = None
_lock = threading.RLock()


def _add_cuda_dlls():
    # CTranslate2 грузит cuBLAS и cuDNN обычным LoadLibrary, а он смотрит только в PATH
    if getattr(sys, "frozen", False):
        dirs = [Path(sys._MEIPASS)]
    else:
        nvidia = Path(sys.executable).parent.parent / "Lib" / "site-packages" / "nvidia"
        dirs = list(nvidia.glob("*/bin"))
    for directory in dirs:
        os.environ["PATH"] = str(directory) + os.pathsep + os.environ["PATH"]


def load(model_name):
    global _model, _model_name
    with _lock:
        if _model is not None and _model_name == model_name:
            return _model

        import ctranslate2
        from faster_whisper import WhisperModel

        _add_cuda_dlls()
        if ctranslate2.get_cuda_device_count() > 0:
            try:
                _model = WhisperModel(model_name, device="cuda", compute_type="float16")
                _model_name = model_name
                return _model
            except Exception:
                logging.exception("Видеокарта есть, но CUDA не поднялась, работаем на CPU")

        _model = WhisperModel(model_name, device="cpu", compute_type="int8")
        _model_name = model_name
        return _model


def unload():
    global _model, _model_name
    with _lock:
        if _model is None:
            return False
        _model = None
        _model_name = None
        gc.collect()
        return True


def is_loaded():
    return _model is not None


def transcribe(audio, model_name, language):
    with _lock:
        model = load(model_name)
        segments, _ = model.transcribe(
            audio,
            language=None if language == "auto" else language,
            beam_size=5,
            vad_filter=True,
            condition_on_previous_text=False,
        )
        # на тишине и шуме Whisper выдумывает текст, поэтому отсеиваем сегменты без речи
        speech = [s.text.strip() for s in segments if s.no_speech_prob < NO_SPEECH_THRESHOLD]
        return " ".join(speech).strip()
