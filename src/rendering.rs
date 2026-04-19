use std::path::Path;

use crate::{
    audio::{AudioBuffer, MonoBuffer},
    config::{
        AmbisonicsChannelOrdering, AmbisonicsConfig, AmbisonicsNormalization,
        AmbisonicsPositioningSpec, PostConvolutionConfig, RenderMode, RenderingConfig,
        StereoRouting,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderPlan {
    pub mode: RenderMode,
    pub stereo_routing: StereoRouting,
    pub ambisonics: AmbisonicsRenderPlan,
    pub post_convolution: PostConvolutionPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostConvolutionPlan {
    pub enabled: bool,
    pub impulse_response: Vec<f32>,
    pub dry_mix: f32,
    pub wet_mix: f32,
    pub normalize_output: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmbisonicsRenderPlan {
    pub order: u8,
    pub channel_ordering: AmbisonicsChannelOrdering,
    pub normalization: AmbisonicsNormalization,
    pub positioning_json_path: Option<String>,
    pub positioning: Option<AmbisonicsPositioningSpec>,
}

impl From<&RenderingConfig> for RenderPlan {
    fn from(config: &RenderingConfig) -> Self {
        Self {
            mode: config.mode,
            stereo_routing: config.stereo_routing,
            ambisonics: AmbisonicsRenderPlan::from(&config.ambisonics),
            post_convolution: PostConvolutionPlan::from(&config.post_convolution),
        }
    }
}

impl From<&AmbisonicsConfig> for AmbisonicsRenderPlan {
    fn from(config: &AmbisonicsConfig) -> Self {
        let positioning_json_path = config.positioning_json_path.trim();

        Self {
            order: config.order,
            channel_ordering: config.channel_ordering,
            normalization: config.normalization,
            positioning_json_path: if positioning_json_path.is_empty() {
                None
            } else {
                Some(positioning_json_path.to_string())
            },
            positioning: None,
        }
    }
}

impl From<&PostConvolutionConfig> for PostConvolutionPlan {
    fn from(config: &PostConvolutionConfig) -> Self {
        Self {
            enabled: config.enabled,
            impulse_response: Vec::new(),
            dry_mix: config.dry_mix,
            wet_mix: config.wet_mix,
            normalize_output: config.normalize_output,
        }
    }
}

pub fn render_reconstruction(
    plan: &RenderPlan,
    reconstruction: &MonoBuffer,
) -> Result<AudioBuffer, String> {
    let processed_samples = apply_post_convolution(&plan.post_convolution, &reconstruction.samples);

    match plan.mode {
        RenderMode::Mono => AudioBuffer::new(reconstruction.sample_rate, 1, processed_samples),
        RenderMode::Stereo => AudioBuffer::new(
            reconstruction.sample_rate,
            2,
            route_stereo(plan.stereo_routing, &processed_samples),
        ),
        RenderMode::AmbisonicsReserved => {
            Err("ambisonics rendering is reserved for a later phase".to_string())
        }
    }
}

pub fn write_output_wav<P>(path: P, mode: RenderMode, buffer: &AudioBuffer) -> Result<(), String>
where
    P: AsRef<Path>,
{
    match mode {
        RenderMode::Mono if buffer.channels != 1 => Err(format!(
            "mono rendering requires 1 channel, found {}",
            buffer.channels
        )),
        RenderMode::Stereo if buffer.channels != 2 => Err(format!(
            "stereo rendering requires 2 channels, found {}",
            buffer.channels
        )),
        RenderMode::AmbisonicsReserved => {
            Err("ambisonics rendering is reserved for a later phase".to_string())
        }
        _ => crate::audio::write_wav(path, buffer),
    }
}

fn route_stereo(routing: StereoRouting, mono_samples: &[f32]) -> Vec<f32> {
    match routing {
        StereoRouting::DuplicateMono => {
            let mut samples = Vec::with_capacity(mono_samples.len() * 2);
            for &sample in mono_samples {
                samples.push(sample);
                samples.push(sample);
            }
            samples
        }
    }
}

fn apply_post_convolution(plan: &PostConvolutionPlan, dry_signal: &[f32]) -> Vec<f32> {
    if !plan.enabled {
        return dry_signal.to_vec();
    }

    if dry_signal.is_empty() {
        return Vec::new();
    }

    let wet_signal = convolve(dry_signal, &plan.impulse_response);
    let mut mixed = vec![0.0; wet_signal.len().max(dry_signal.len())];

    for (index, &sample) in dry_signal.iter().enumerate() {
        mixed[index] += sample * plan.dry_mix;
    }
    for (index, &sample) in wet_signal.iter().enumerate() {
        mixed[index] += sample * plan.wet_mix;
    }

    if plan.normalize_output {
        normalize_output_in_place(&mut mixed);
    }

    mixed
}

fn convolve(signal: &[f32], impulse_response: &[f32]) -> Vec<f32> {
    if signal.is_empty() || impulse_response.is_empty() {
        return Vec::new();
    }

    let mut output = vec![0.0; signal.len() + impulse_response.len() - 1];

    for (signal_index, &signal_sample) in signal.iter().enumerate() {
        for (impulse_index, &impulse_sample) in impulse_response.iter().enumerate() {
            output[signal_index + impulse_index] += signal_sample * impulse_sample;
        }
    }

    output
}

fn normalize_output_in_place(samples: &mut [f32]) {
    let peak = samples.iter().fold(0.0_f32, |current_peak, sample| {
        current_peak.max(sample.abs())
    });

    if peak <= 1.0 {
        return;
    }

    let scale = 1.0 / peak;
    for sample in samples {
        *sample *= scale;
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{AmbisonicsRenderPlan, PostConvolutionPlan, RenderPlan, render_reconstruction};
    use crate::{
        audio::MonoBuffer,
        config::{AppConfig, RenderMode, StereoRouting},
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn render_reconstruction_keeps_mono_signal_in_mono_mode() {
        let plan = RenderPlan {
            mode: RenderMode::Mono,
            stereo_routing: StereoRouting::DuplicateMono,
            ambisonics: disabled_ambisonics(),
            post_convolution: disabled_post_convolution(),
        };
        let reconstruction = MonoBuffer::new(48_000, vec![0.25, -0.5, 0.75]).expect("buffer");

        let rendered = render_reconstruction(&plan, &reconstruction).expect("rendering");

        assert_eq!(rendered.channels, 1);
        assert_eq!(rendered.samples, vec![0.25, -0.5, 0.75]);
    }

    #[test]
    fn render_reconstruction_duplicates_mono_signal_for_stereo_output() {
        let plan = RenderPlan {
            mode: RenderMode::Stereo,
            stereo_routing: StereoRouting::DuplicateMono,
            ambisonics: disabled_ambisonics(),
            post_convolution: disabled_post_convolution(),
        };
        let reconstruction = MonoBuffer::new(48_000, vec![0.25, -0.5]).expect("buffer");

        let rendered = render_reconstruction(&plan, &reconstruction).expect("rendering");

        assert_eq!(rendered.channels, 2);
        assert_eq!(rendered.samples, vec![0.25, 0.25, -0.5, -0.5]);
    }

    #[test]
    fn render_reconstruction_applies_post_convolution_and_extends_tail() {
        let plan = RenderPlan {
            mode: RenderMode::Mono,
            stereo_routing: StereoRouting::DuplicateMono,
            ambisonics: disabled_ambisonics(),
            post_convolution: PostConvolutionPlan {
                enabled: true,
                impulse_response: vec![0.5, 0.5],
                dry_mix: 0.0,
                wet_mix: 1.0,
                normalize_output: false,
            },
        };
        let reconstruction = MonoBuffer::new(1_000, vec![1.0, 0.0]).expect("buffer");

        let rendered = render_reconstruction(&plan, &reconstruction).expect("rendering");

        assert_eq!(rendered.channels, 1);
        assert_eq!(rendered.samples, vec![0.5, 0.5, 0.0]);
    }

    #[test]
    fn render_reconstruction_normalizes_post_convolution_peak() {
        let plan = RenderPlan {
            mode: RenderMode::Mono,
            stereo_routing: StereoRouting::DuplicateMono,
            ambisonics: disabled_ambisonics(),
            post_convolution: PostConvolutionPlan {
                enabled: true,
                impulse_response: vec![0.5, 0.5],
                dry_mix: 1.0,
                wet_mix: 1.0,
                normalize_output: true,
            },
        };
        let reconstruction = MonoBuffer::new(1_000, vec![1.0, 0.0]).expect("buffer");

        let rendered = render_reconstruction(&plan, &reconstruction).expect("rendering");

        let expected = [1.0, 1.0 / 3.0, 0.0];
        for (actual, expected) in rendered.samples.iter().zip(expected) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
    }

    #[test]
    fn render_reconstruction_rejects_reserved_ambisonics_mode() {
        let plan = RenderPlan {
            mode: RenderMode::AmbisonicsReserved,
            stereo_routing: StereoRouting::DuplicateMono,
            ambisonics: disabled_ambisonics(),
            post_convolution: disabled_post_convolution(),
        };
        let reconstruction = MonoBuffer::new(48_000, vec![0.25]).expect("buffer");

        let error = render_reconstruction(&plan, &reconstruction).expect_err("rendering must fail");

        assert_eq!(error, "ambisonics rendering is reserved for a later phase");
    }

    #[test]
    fn render_reconstruction_attempts_ambisonics_with_example_json() {
        let fixture = TempFixtureDir::new();
        let json_path = fixture.write_text_file(
            "ambisonics-positioning.json",
            r#"{
  "space": "cartesian",
  "default_curve": "linear",
  "trajectory": [
    {
      "time_ms": 0,
      "position": {
        "x": 0.0,
        "y": 1.0,
        "z": 0.0
      },
      "to_next": {
        "curve": "linear"
      }
    },
    {
      "time_ms": 120,
      "position": {
        "x": 0.5,
        "y": 0.4,
        "z": 0.1
      }
    }
  ],
  "jitter": {
    "mode": "gaussian",
    "per_grain": true,
    "seed": 13,
    "spread": {
      "x": 0.08,
      "y": 0.08,
      "z": 0.04
    },
    "smoothing_ms": 80
  }
}"#,
        );
        let mut config = AppConfig::default();
        config.rendering.mode = RenderMode::AmbisonicsReserved;
        config.rendering.ambisonics.positioning_json_path =
            json_path.to_string_lossy().into_owned();
        config
            .validate()
            .expect("ambisonics config should validate");

        let plan = RenderPlan::from(&config.rendering);
        assert_eq!(plan.ambisonics.order, 1);
        assert_eq!(plan.ambisonics.channel_ordering.as_str(), "acn");
        assert_eq!(plan.ambisonics.normalization.as_str(), "sn3d");
        assert_eq!(
            plan.ambisonics.positioning_json_path,
            Some(json_path.to_string_lossy().into_owned())
        );

        let reconstruction = MonoBuffer::new(48_000, vec![0.25, -0.5, 0.75]).expect("buffer");
        let error = render_reconstruction(&plan, &reconstruction)
            .expect_err("ambisonics render should stay reserved");

        assert_eq!(error, "ambisonics rendering is reserved for a later phase");
    }

    fn disabled_post_convolution() -> PostConvolutionPlan {
        PostConvolutionPlan {
            enabled: false,
            impulse_response: Vec::new(),
            dry_mix: 1.0,
            wet_mix: 1.0,
            normalize_output: true,
        }
    }

    fn disabled_ambisonics() -> AmbisonicsRenderPlan {
        AmbisonicsRenderPlan {
            order: 1,
            channel_ordering: crate::config::AmbisonicsChannelOrdering::Acn,
            normalization: crate::config::AmbisonicsNormalization::Sn3d,
            positioning_json_path: None,
            positioning: None,
        }
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
                "corpusflow-rendering-{}-{}-{}",
                std::process::id(),
                nanos,
                unique
            ));

            fs::create_dir_all(&path).expect("temp fixture dir should be created");
            Self { path }
        }

        fn write_text_file(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture parent dir should be created");
            }

            fs::write(&path, contents).expect("fixture file should be written");
            path
        }
    }

    impl Drop for TempFixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
