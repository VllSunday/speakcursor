use std::io;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, SampleFormat, Stream};

use crate::resample_mono;

pub struct AudioCapture {
    stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    stream_error: Arc<Mutex<Option<String>>>,
    activity: Arc<AtomicU8>,
    sample_rate: u32,
}

impl AudioCapture {
    pub fn start_default() -> io::Result<Self> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "microphone not found"))?;
        let supported = device.default_input_config().map_err(io::Error::other)?;
        let sample_rate = supported.sample_rate();
        let channels = supported.channels() as usize;
        if channels == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "microphone has no channels",
            ));
        }
        let config: cpal::StreamConfig = supported.clone().into();
        // Keep routine allocations away from the realtime audio callback.
        let samples = Arc::new(Mutex::new(Vec::with_capacity(sample_rate as usize * 30)));
        let stream_error = Arc::new(Mutex::new(None));
        let activity = Arc::new(AtomicU8::new(0));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => {
                let output = samples.clone();
                let errors = stream_error.clone();
                let activity_meter = activity.clone();
                device.build_input_stream(
                    config.clone(),
                    move |data: &[f32], _| {
                        append_mono(data, channels, &output, &activity_meter, |sample| sample)
                    },
                    move |error| record_stream_error(error, &errors),
                    None,
                )
            }
            SampleFormat::I16 => {
                let output = samples.clone();
                let errors = stream_error.clone();
                let activity_meter = activity.clone();
                device.build_input_stream(
                    config.clone(),
                    move |data: &[i16], _| {
                        append_mono(data, channels, &output, &activity_meter, |sample| {
                            sample as f32 / 32768.0
                        })
                    },
                    move |error| record_stream_error(error, &errors),
                    None,
                )
            }
            SampleFormat::U16 => {
                let output = samples.clone();
                let errors = stream_error.clone();
                let activity_meter = activity.clone();
                device.build_input_stream(
                    config.clone(),
                    move |data: &[u16], _| {
                        append_mono(data, channels, &output, &activity_meter, |sample| {
                            (sample as f32 - 32768.0) / 32768.0
                        })
                    },
                    move |error| record_stream_error(error, &errors),
                    None,
                )
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("microphone format {other} is unsupported"),
                ));
            }
        }
        .map_err(io::Error::other)?;
        stream.play().map_err(io::Error::other)?;
        Ok(Self {
            stream,
            samples,
            stream_error,
            activity,
            sample_rate,
        })
    }

    /// Recent microphone level. Zero means no input callback has delivered speech.
    pub fn take_activity(&self) -> u8 {
        self.activity.swap(0, Ordering::Relaxed)
    }

    pub fn finish(self) -> io::Result<Vec<f32>> {
        drop(self.stream);
        if let Some(error) = self.stream_error.lock().unwrap().take() {
            return Err(io::Error::other(error));
        }
        let mut samples = self.samples.lock().unwrap();
        let captured = std::mem::take(&mut *samples);
        drop(samples);
        if captured.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Микрофон не передал звук. Проверь выбранное устройство ввода Windows",
            ));
        }
        resample_mono(&captured, self.sample_rate)
    }
}

fn record_stream_error(error: cpal::Error, errors: &Mutex<Option<String>>) {
    // WASAPI may report a recoverable xrun while continuing to deliver input.
    // The previous code discarded the entire recording on the first xrun.
    if error.kind() == ErrorKind::Xrun {
        return;
    }
    let mut fatal = errors.lock().unwrap();
    if fatal.is_none() {
        *fatal = Some(error.to_string());
    }
}

fn append_mono<T: Copy>(
    data: &[T],
    channels: usize,
    output: &Mutex<Vec<f32>>,
    activity: &AtomicU8,
    to_f32: impl Fn(T) -> f32,
) {
    let mut samples = output.lock().unwrap();
    let mut peak = 0.0_f32;
    for frame in data.chunks_exact(channels) {
        let mono = frame.iter().map(|sample| to_f32(*sample)).sum::<f32>() / channels as f32;
        let mono = mono.clamp(-1.0, 1.0);
        peak = peak.max(mono.abs());
        samples.push(mono);
    }
    let level = (peak.sqrt() * 8.0).round().clamp(0.0, 8.0) as u8;
    activity.fetch_max(level, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_xrun_does_not_discard_captured_audio() {
        let errors = Mutex::new(None);
        record_stream_error(cpal::Error::new(ErrorKind::Xrun), &errors);
        assert!(errors.lock().unwrap().is_none());
    }

    #[test]
    fn permanent_stream_error_is_preserved() {
        let errors = Mutex::new(None);
        record_stream_error(cpal::Error::new(ErrorKind::DeviceNotAvailable), &errors);
        assert!(
            errors
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .contains("not available")
        );
    }
}
