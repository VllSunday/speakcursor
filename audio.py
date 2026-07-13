import logging

import numpy as np
import sounddevice as sd

SAMPLE_RATE = 16000
MIN_DURATION = 0.3

_stream = None
_frames = []


def start():
    global _stream, _frames
    if _stream is not None:
        return
    _frames = []

    def callback(indata, frames_count, time_info, status):
        _frames.append(indata.copy())

    _stream = sd.InputStream(samplerate=SAMPLE_RATE, channels=1, dtype="float32", callback=callback)
    _stream.start()


def stop():
    global _stream
    if _stream is None:
        return None
    try:
        _stream.stop()
        _stream.close()
    except Exception:
        logging.exception("Ошибка при остановке записи")
    _stream = None

    if not _frames:
        return None
    audio = np.concatenate(_frames, axis=0).flatten()
    if len(audio) < SAMPLE_RATE * MIN_DURATION:
        return None
    return audio
