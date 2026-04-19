use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use corpusflow::{
    audio::{AudioBuffer, read_wav},
    config::{CorpusConfig, RenderMode, TargetConfig},
    corpus::CorpusPlan,
    rendering::write_output_wav,
    target::TargetInput,
};
use hound::{SampleFormat, WavSpec, WavWriter};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn loads_corpus_wavs_recursively_in_sorted_order() {
    let fixture = TempFixtureDir::new();
    fixture.create_dir("nested");
    fixture.write_pcm16_wav("b.wav", 1, &[0, 16_384, -16_384]);
    fixture.write_text("ignore.txt", "not audio");
    fixture.write_pcm16_wav("nested/a.wav", 1, &[32_767, -32_768]);

    let config = CorpusConfig {
        root: fixture.path().to_string_lossy().into_owned(),
        grain_size_ms: 100,
        grain_hop_ms: 50,
        mono_only: true,
    };

    let corpus = CorpusPlan::from_config(&config)
        .load_sources(&config.root)
        .expect("corpus should load");
    let paths = corpus
        .iter()
        .map(|item| {
            item.path
                .strip_prefix(fixture.path())
                .expect("path should be under the fixture root")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();

    assert_eq!(paths, vec!["b.wav".to_string(), "nested/a.wav".to_string()]);
    assert_eq!(corpus[0].audio.frame_count(), 3);
    assert_eq!(corpus[1].audio.frame_count(), 2);
}

#[test]
fn downmixes_stereo_corpus_input_to_mono() {
    let fixture = TempFixtureDir::new();
    let path = fixture.write_pcm16_wav("stereo.wav", 2, &[0, 10_000, 10_000, -10_000]);

    let config = CorpusConfig {
        root: path.to_string_lossy().into_owned(),
        grain_size_ms: 100,
        grain_hop_ms: 50,
        mono_only: true,
    };

    let corpus = CorpusPlan::from_config(&config)
        .load_sources(&config.root)
        .expect("stereo corpus should downmix");

    assert_eq!(corpus.len(), 1);
    assert_eq!(corpus[0].audio.frame_count(), 2);
    assert!(approx_eq(corpus[0].audio.samples[0], 5_000.0 / 32_767.0));
    assert!(approx_eq(corpus[0].audio.samples[1], 0.0));
}

#[test]
fn loads_stereo_target_audio() {
    let fixture = TempFixtureDir::new();
    let path = fixture.write_pcm16_wav("target.wav", 2, &[0, 16_384, -16_384, 8_192]);

    let config = TargetConfig {
        path: path.to_string_lossy().into_owned(),
        frame_size_ms: 100,
        hop_size_ms: 50,
    };

    let target = TargetInput::load(&config).expect("target should load");

    assert_eq!(target.audio.channels, 2);
    assert_eq!(target.audio.frame_count(), 2);
    assert!(approx_eq(target.audio.samples[1], 16_384.0 / 32_767.0));
    assert!(approx_eq(target.audio.samples[2], -16_384.0 / 32_767.0));
}

#[test]
fn writes_and_reads_back_stereo_output() {
    let fixture = TempFixtureDir::new();
    let path = fixture.path().join("output.wav");
    let buffer = AudioBuffer::new(48_000, 2, vec![0.0, 0.5, -0.25, 0.25]).expect("buffer");

    write_output_wav(&path, RenderMode::Stereo, &buffer).expect("write should succeed");
    let roundtrip = read_wav(&path).expect("roundtrip should load");

    assert_eq!(roundtrip.sample_rate, 48_000);
    assert_eq!(roundtrip.channels, 2);
    assert_eq!(roundtrip.samples, vec![0.0, 0.5, -0.25, 0.25]);
}

#[test]
fn rejects_output_channel_mismatch() {
    let fixture = TempFixtureDir::new();
    let path = fixture.path().join("mono.wav");
    let buffer = AudioBuffer::new(48_000, 1, vec![0.0, 0.5]).expect("buffer");

    let error =
        write_output_wav(&path, RenderMode::Stereo, &buffer).expect_err("mismatch must fail");

    assert_eq!(error, "stereo rendering requires 2 channels, found 1");
}

fn approx_eq(left: f32, right: f32) -> bool {
    (left - right).abs() < 1.0e-6
}

struct TempFixtureDir {
    path: PathBuf,
}

impl TempFixtureDir {
    fn new() -> Self {
        let unique = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "corpusflow-audio-io-{}-{}-{}",
            std::process::id(),
            nanos,
            unique
        ));

        fs::create_dir_all(&path).expect("temp fixture dir should be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn create_dir(&self, relative: &str) {
        fs::create_dir_all(self.path.join(relative)).expect("fixture subdir should be created");
    }

    fn write_text(&self, relative: &str, contents: &str) {
        fs::write(self.path.join(relative), contents).expect("fixture text file should be written");
    }

    fn write_pcm16_wav(&self, relative: &str, channels: u16, samples: &[i16]) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent dir should be created");
        }

        let spec = WavSpec {
            channels,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(&path, spec).expect("fixture WAV should be created");

        for &sample in samples {
            writer
                .write_sample(sample)
                .expect("fixture sample should be written");
        }

        writer.finalize().expect("fixture WAV should finalize");
        path
    }
}

impl Drop for TempFixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
