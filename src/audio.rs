use std::path::Path;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl AudioBuffer {
    pub fn new(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Result<Self, String> {
        let buffer = Self {
            sample_rate,
            channels,
            samples,
        };
        buffer.validate()?;
        Ok(buffer)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.sample_rate == 0 {
            return Err("audio sample_rate must be > 0".to_string());
        }
        if self.channels == 0 {
            return Err("audio channels must be > 0".to_string());
        }
        if self.samples.len() % self.channels as usize != 0 {
            return Err("audio samples must align with channel count".to_string());
        }

        Ok(())
    }

    pub fn frame_count(&self) -> usize {
        self.samples.len() / self.channels as usize
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonoBuffer {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl MonoBuffer {
    pub fn new(sample_rate: u32, samples: Vec<f32>) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("mono sample_rate must be > 0".to_string());
        }

        Ok(Self {
            sample_rate,
            samples,
        })
    }

    pub fn frame_count(&self) -> usize {
        self.samples.len()
    }
}

impl TryFrom<AudioBuffer> for MonoBuffer {
    type Error = String;

    fn try_from(buffer: AudioBuffer) -> Result<Self, Self::Error> {
        if buffer.channels != 1 {
            return Err(format!(
                "expected mono WAV input, found {} channels",
                buffer.channels
            ));
        }

        MonoBuffer::new(buffer.sample_rate, buffer.samples)
    }
}

impl From<MonoBuffer> for AudioBuffer {
    fn from(buffer: MonoBuffer) -> Self {
        Self {
            sample_rate: buffer.sample_rate,
            channels: 1,
            samples: buffer.samples,
        }
    }
}

pub fn read_wav<P>(path: P) -> Result<AudioBuffer, String>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let mut reader = WavReader::open(path)
        .map_err(|error| format!("failed to open WAV `{}`: {error}", path.display()))?;
    let spec = reader.spec();

    let samples = match spec.sample_format {
        SampleFormat::Float => read_float_samples(&mut reader, spec.bits_per_sample, path)?,
        SampleFormat::Int => read_int_samples(&mut reader, spec.bits_per_sample, path)?,
    };

    AudioBuffer::new(spec.sample_rate, spec.channels, samples)
        .map_err(|error| format!("invalid WAV `{}`: {error}", path.display()))
}

pub fn read_mono_wav<P>(path: P) -> Result<MonoBuffer, String>
where
    P: AsRef<Path>,
{
    MonoBuffer::try_from(read_wav(path)?)
}

pub fn write_wav<P>(path: P, buffer: &AudioBuffer) -> Result<(), String>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    buffer.validate()?;

    if buffer.channels != 1 && buffer.channels != 2 {
        return Err(format!(
            "WAV writer supports only mono or stereo output, found {} channels",
            buffer.channels
        ));
    }

    let spec = WavSpec {
        channels: buffer.channels,
        sample_rate: buffer.sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer = WavWriter::create(path, spec)
        .map_err(|error| format!("failed to create WAV `{}`: {error}", path.display()))?;

    for &sample in &buffer.samples {
        writer
            .write_sample(sample.clamp(-1.0, 1.0))
            .map_err(|error| format!("failed to write WAV `{}`: {error}", path.display()))?;
    }

    writer
        .finalize()
        .map_err(|error| format!("failed to finalize WAV `{}`: {error}", path.display()))
}

fn read_float_samples(
    reader: &mut WavReader<std::io::BufReader<std::fs::File>>,
    bits_per_sample: u16,
    path: &Path,
) -> Result<Vec<f32>, String> {
    if bits_per_sample != 32 {
        return Err(format!(
            "unsupported float WAV `{}`: expected 32-bit float, found {bits_per_sample}-bit",
            path.display()
        ));
    }

    reader
        .samples::<f32>()
        .map(|sample| {
            sample.map_err(|error| format!("failed to read WAV `{}`: {error}", path.display()))
        })
        .collect()
}

fn read_int_samples(
    reader: &mut WavReader<std::io::BufReader<std::fs::File>>,
    bits_per_sample: u16,
    path: &Path,
) -> Result<Vec<f32>, String> {
    if bits_per_sample == 0 || bits_per_sample > 32 {
        return Err(format!(
            "unsupported integer WAV `{}`: found {bits_per_sample}-bit PCM",
            path.display()
        ));
    }

    if bits_per_sample == 8 {
        return Err(format!(
            "unsupported integer WAV `{}`: 8-bit PCM is not enabled in phase 01",
            path.display()
        ));
    }

    let scale = ((1_i64 << (bits_per_sample - 1)) - 1) as f32;

    reader
        .samples::<i32>()
        .map(|sample| {
            sample
                .map(|value| (value as f32 / scale).clamp(-1.0, 1.0))
                .map_err(|error| format!("failed to read WAV `{}`: {error}", path.display()))
        })
        .collect()
}
