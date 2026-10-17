use std::io;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::resample_mono;

pub struct AudioCapture {
    stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    stream_error: Arc<Mutex<Option<String>>>,
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
        let samples = Arc::new(Mutex::new(Vec::new()));
        let stream_error = Arc::new(Mutex::new(None));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => {
                let output = samples.clone();
                let errors = stream_error.clone();
                device.build_input_stream(
                    config.clone(),
                    move |data: &[f32], _| append_mono(data, channels, &output, |sample| sample),
                    move |error| *errors.lock().unwrap() = Some(error.to_string()),
                    None,
                )
            }
            SampleFormat::I16 => {
                let output = samples.clone();
                let errors = stream_error.clone();
                device.build_input_stream(
                    config.clone(),
                    move |data: &[i16], _| {
                        append_mono(data, channels, &output, |sample| sample as f32 / 32768.0)
                    },
                    move |error| *errors.lock().unwrap() = Some(error.to_string()),
                    None,
                )
            }
            SampleFormat::U16 => {
                let output = samples.clone();
                let errors = stream_error.clone();
                device.build_input_stream(
                    config.clone(),
                    move |data: &[u16], _| {
                        append_mono(data, channels, &output, |sample| {
                            (sample as f32 - 32768.0) / 32768.0
                        })
                    },
                    move |error| *errors.lock().unwrap() = Some(error.to_string()),
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
            sample_rate,
        })
    }

    pub fn finish(self) -> io::Result<Vec<f32>> {
        drop(self.stream);
        if let Some(error) = self.stream_error.lock().unwrap().take() {
            return Err(io::Error::other(error));
        }
        let mut samples = self.samples.lock().unwrap();
        let captured = std::mem::take(&mut *samples);
        drop(samples);
        resample_mono(&captured, self.sample_rate)
    }
}

fn append_mono<T: Copy>(
    data: &[T],
    channels: usize,
    output: &Mutex<Vec<f32>>,
    to_f32: impl Fn(T) -> f32,
) {
    let mut samples = output.lock().unwrap();
    for frame in data.chunks_exact(channels) {
        let mono = frame.iter().map(|sample| to_f32(*sample)).sum::<f32>() / channels as f32;
        samples.push(mono.clamp(-1.0, 1.0));
    }
}
