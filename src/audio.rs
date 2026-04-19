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

    pub fn resample_to(&self, output_sample_rate: u32) -> Result<Self, String> {
        if output_sample_rate == self.sample_rate {
            return Ok(self.clone());
        }

        if output_sample_rate == 0 {
            return Err("audio resample output_sample_rate must be > 0".to_string());
        }

        let output_samples = resample_interleaved(
            &self.samples,
            self.channels as usize,
            self.sample_rate,
            output_sample_rate,
        );

        Self::new(output_sample_rate, self.channels, output_samples)
    }

    pub fn into_mono_downmix(self) -> Result<MonoBuffer, String> {
        match self.channels {
            1 => MonoBuffer::new(self.sample_rate, self.samples),
            2 => {
                let mut mono_samples = Vec::with_capacity(self.frame_count());

                for frame in self.samples.chunks_exact(2) {
                    mono_samples.push((frame[0] + frame[1]) * 0.5);
                }

                MonoBuffer::new(self.sample_rate, mono_samples)
            }
            channels => Err(format!(
                "corpus mono downmix supports only mono or stereo WAV input, found {channels} channels"
            )),
        }
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

    pub fn resample_to(&self, output_sample_rate: u32) -> Result<Self, String> {
        if output_sample_rate == self.sample_rate {
            return Ok(self.clone());
        }

        if output_sample_rate == 0 {
            return Err("mono resample output_sample_rate must be > 0".to_string());
        }

        let output_samples =
            resample_interleaved(&self.samples, 1, self.sample_rate, output_sample_rate);
        Self::new(output_sample_rate, output_samples)
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

pub fn read_corpus_mono_wav<P>(path: P) -> Result<MonoBuffer, String>
where
    P: AsRef<Path>,
{
    read_wav(path)?.into_mono_downmix()
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

fn resample_interleaved(
    input_samples: &[f32],
    channels: usize,
    input_sample_rate: u32,
    output_sample_rate: u32,
) -> Vec<f32> {
    if input_samples.is_empty() {
        return Vec::new();
    }

    let input_frames = input_samples.len() / channels;
    if input_frames == 0 {
        return Vec::new();
    }

    let output_frames = ((input_frames as u64 * output_sample_rate as u64)
        + (input_sample_rate as u64 / 2))
        / input_sample_rate as u64;
    let output_frames = output_frames.max(1) as usize;
    let mut output_samples = vec![0.0; output_frames * channels];

    for output_frame in 0..output_frames {
        let source_numerator = output_frame as u64 * input_sample_rate as u64;
        let source_index = (source_numerator / output_sample_rate as u64) as usize;

        if source_index >= input_frames.saturating_sub(1) {
            let input_offset = (input_frames - 1) * channels;
            let output_offset = output_frame * channels;
            output_samples[output_offset..output_offset + channels]
                .copy_from_slice(&input_samples[input_offset..input_offset + channels]);
            continue;
        }

        let next_index = source_index + 1;
        let fractional =
            (source_numerator % output_sample_rate as u64) as f32 / output_sample_rate as f32;
        let current_offset = source_index * channels;
        let next_offset = next_index * channels;
        let output_offset = output_frame * channels;

        for channel in 0..channels {
            let current = input_samples[current_offset + channel];
            let next = input_samples[next_offset + channel];
            output_samples[output_offset + channel] = current + (next - current) * fractional;
        }
    }

    output_samples
}

#[cfg(test)]
mod tests {
    use super::{AudioBuffer, MonoBuffer};

    #[test]
    fn downmixes_stereo_audio_to_mono_by_frame_average() {
        let buffer = AudioBuffer::new(48_000, 2, vec![0.25, 0.75, -1.0, 0.5]).expect("buffer");

        let mono = buffer.into_mono_downmix().expect("stereo should downmix");

        assert_eq!(mono.sample_rate, 48_000);
        assert_eq!(mono.samples, vec![0.5, -0.25]);
    }

    #[test]
    fn rejects_multichannel_downmix_beyond_stereo() {
        let buffer = AudioBuffer::new(48_000, 3, vec![0.0, 0.1, 0.2]).expect("buffer");

        let error = buffer
            .into_mono_downmix()
            .expect_err("3-channel corpus audio must fail");

        assert_eq!(
            error,
            "corpus mono downmix supports only mono or stereo WAV input, found 3 channels"
        );
    }

    #[test]
    fn resamples_mono_buffer_with_linear_interpolation() {
        let buffer = MonoBuffer::new(2, vec![0.0, 1.0]).expect("buffer");

        let resampled = buffer.resample_to(4).expect("resampled mono");

        assert_eq!(resampled.sample_rate, 4);
        assert_eq!(resampled.samples.len(), 4);
        assert!((resampled.samples[0] - 0.0).abs() < 1.0e-6);
        assert!((resampled.samples[1] - 0.5).abs() < 1.0e-6);
        assert!((resampled.samples[2] - 1.0).abs() < 1.0e-6);
        assert!((resampled.samples[3] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn resamples_multichannel_audio_per_channel() {
        let buffer = AudioBuffer::new(2, 2, vec![0.0, 10.0, 1.0, 20.0]).expect("stereo buffer");

        let resampled = buffer.resample_to(4).expect("resampled stereo");

        assert_eq!(resampled.sample_rate, 4);
        assert_eq!(resampled.channels, 2);
        assert_eq!(resampled.samples.len(), 8);
        assert!((resampled.samples[0] - 0.0).abs() < 1.0e-6);
        assert!((resampled.samples[1] - 10.0).abs() < 1.0e-6);
        assert!((resampled.samples[2] - 0.5).abs() < 1.0e-6);
        assert!((resampled.samples[3] - 15.0).abs() < 1.0e-6);
        assert!((resampled.samples[4] - 1.0).abs() < 1.0e-6);
        assert!((resampled.samples[5] - 20.0).abs() < 1.0e-6);
        assert!((resampled.samples[6] - 1.0).abs() < 1.0e-6);
        assert!((resampled.samples[7] - 20.0).abs() < 1.0e-6);
    }
}
