//! Microphone capture and 16 kHz mono preparation for dictation.

use std::io;
use std::io::Cursor;

pub const TRANSCRIPTION_SAMPLE_RATE: u32 = 16_000;

#[cfg(windows)]
mod capture;
#[cfg(windows)]
pub use capture::AudioCapture;
#[cfg(windows)]
mod playback;
#[cfg(windows)]
pub use playback::{play_wav_blocking, stop_playback};

/// Downmixing is performed in the input callback; this converts mono samples
/// from the device's native rate to the model/API rate. Averaging over each
/// output sample interval reduces aliasing when a 48 kHz device feeds 16 kHz.
pub fn resample_mono(input: &[f32], source_rate: u32) -> io::Result<Vec<f32>> {
    if source_rate == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "zero sample rate",
        ));
    }
    if source_rate == TRANSCRIPTION_SAMPLE_RATE {
        return Ok(input.to_vec());
    }
    let count = input.len() as u64 * TRANSCRIPTION_SAMPLE_RATE as u64 / source_rate as u64;
    let mut output = Vec::with_capacity(count as usize);
    let interval = source_rate as f64 / TRANSCRIPTION_SAMPLE_RATE as f64;
    for i in 0..count {
        let mut position = i as f64 * interval;
        let end = (i + 1) as f64 * interval;
        let mut sum = 0.0_f64;
        while position < end {
            let source_index = position.floor() as usize;
            let next = (source_index as f64 + 1.0).min(end);
            sum += input[source_index] as f64 * (next - position);
            position = next;
        }
        output.push((sum / interval).clamp(-1.0, 1.0) as f32);
    }
    Ok(output)
}

pub fn encode_wav_16khz_mono(samples: &[f32]) -> io::Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TRANSCRIPTION_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut bytes = Vec::new();
    {
        let mut writer =
            hound::WavWriter::new(Cursor::new(&mut bytes), spec).map_err(io::Error::other)?;
        for sample in samples {
            if !sample.is_finite() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "non-finite audio sample",
                ));
            }
            let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            writer.write_sample(pcm).map_err(io::Error::other)?;
        }
        writer.finalize().map_err(io::Error::other)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_stereo_device_rate_independent_audio_to_16khz_wav() {
        let samples = resample_mono(&[1.0, 0.0, -1.0, 0.5, 0.5, 0.5], 48_000).unwrap();
        assert_eq!(samples.len(), 2);
        assert!(samples[0].abs() < 0.001);
        let wav = encode_wav_16khz_mono(&samples).unwrap();
        let reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.duration(), 2);
    }
}
