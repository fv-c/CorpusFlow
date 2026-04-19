use std::path::Path;

use crate::{
    audio::{AudioBuffer, MonoBuffer},
    config::{
        AmbisonicsCartesianPosition, AmbisonicsChannelOrdering, AmbisonicsConfig, AmbisonicsCurve,
        AmbisonicsNormalization, AmbisonicsPositionJitter, AmbisonicsPositioningSpec,
        PostConvolutionConfig, RenderMode, RenderingConfig, StereoRouting,
    },
};

const SQRT_HALF: f32 = std::f32::consts::FRAC_1_SQRT_2;
const SQRT_THREE: f32 = 1.732_050_8;

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
            render_foa(plan, reconstruction.sample_rate, &processed_samples)
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
        RenderMode::AmbisonicsReserved if buffer.channels < 4 => Err(format!(
            "ambisonics rendering requires at least 4 channels, found {}",
            buffer.channels
        )),
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

fn render_foa(
    plan: &RenderPlan,
    sample_rate: u32,
    mono_samples: &[f32],
) -> Result<AudioBuffer, String> {
    let ambisonics = &plan.ambisonics;
    if ambisonics.order != 1 {
        return Err("ambisonics rendering currently supports only order = 1".to_string());
    }

    let positioning = ambisonics
        .positioning
        .as_ref()
        .ok_or_else(|| "ambisonics rendering requires a loaded positioning spec".to_string())?;

    let mut samples = Vec::with_capacity(mono_samples.len() * 4);
    for (frame_index, &sample) in mono_samples.iter().enumerate() {
        let position = sampled_position(positioning, sample_rate, frame_index);
        let gains = encode_foa_acn(ambisonics.normalization, position);
        for gain in gains {
            samples.push(sample * gain);
        }
    }

    AudioBuffer::new(sample_rate, 4, samples)
}

fn sampled_position(
    positioning: &AmbisonicsPositioningSpec,
    sample_rate: u32,
    frame_index: usize,
) -> AmbisonicsCartesianPosition {
    let base_position = trajectory_position_at_frame(positioning, sample_rate, frame_index);
    let jitter = jitter_offset(&positioning.jitter, sample_rate, frame_index);

    AmbisonicsCartesianPosition {
        x: base_position.x + jitter.x,
        y: base_position.y + jitter.y,
        z: base_position.z + jitter.z,
    }
}

fn trajectory_position_at_frame(
    positioning: &AmbisonicsPositioningSpec,
    sample_rate: u32,
    frame_index: usize,
) -> AmbisonicsCartesianPosition {
    let trajectory = &positioning.trajectory;
    if trajectory.len() == 1 {
        return trajectory[0].position.clone();
    }

    let last_time_ms = trajectory
        .last()
        .map(|waypoint| waypoint.time_ms)
        .unwrap_or(0);
    let time_ms = if positioning.loop_enabled && last_time_ms > 0 {
        let loop_frames = ms_to_frames(sample_rate, last_time_ms).max(1);
        frames_to_ms(sample_rate, frame_index % loop_frames)
    } else {
        frames_to_ms(sample_rate, frame_index)
    };

    if time_ms <= trajectory[0].time_ms as f32 {
        return trajectory[0].position.clone();
    }

    if time_ms >= trajectory[trajectory.len() - 1].time_ms as f32 {
        return trajectory[trajectory.len() - 1].position.clone();
    }

    let segment_index = trajectory
        .windows(2)
        .position(|pair| time_ms >= pair[0].time_ms as f32 && time_ms <= pair[1].time_ms as f32)
        .unwrap_or(trajectory.len() - 2);

    let current = &trajectory[segment_index];
    let next = &trajectory[segment_index + 1];
    let duration_ms = (next.time_ms - current.time_ms) as f32;
    let progress = if duration_ms <= 0.0 {
        0.0
    } else {
        ((time_ms - current.time_ms as f32) / duration_ms).clamp(0.0, 1.0)
    };
    let segment = current.to_next.as_ref();
    let curve = segment
        .map(|segment| segment.curve)
        .unwrap_or(positioning.default_curve);

    match curve {
        AmbisonicsCurve::Hold => current.position.clone(),
        AmbisonicsCurve::Linear => lerp_position(&current.position, &next.position, progress),
        AmbisonicsCurve::CatmullRom => {
            let previous = if segment_index > 0 {
                &trajectory[segment_index - 1].position
            } else {
                &current.position
            };
            let following = if segment_index + 2 < trajectory.len() {
                &trajectory[segment_index + 2].position
            } else {
                &next.position
            };
            let tension = segment.and_then(|segment| segment.tension).unwrap_or(0.5);
            catmull_rom_position(
                previous,
                &current.position,
                &next.position,
                following,
                progress,
                tension,
            )
        }
    }
}

fn jitter_offset(
    jitter: &AmbisonicsPositionJitter,
    sample_rate: u32,
    frame_index: usize,
) -> AmbisonicsCartesianPosition {
    if !jitter.per_grain {
        return AmbisonicsCartesianPosition {
            x: noise_value(jitter.seed.unwrap_or(0), 0, 0) * jitter.spread.x,
            y: noise_value(jitter.seed.unwrap_or(0), 1, 0) * jitter.spread.y,
            z: noise_value(jitter.seed.unwrap_or(0), 2, 0) * jitter.spread.z,
        };
    }

    let interval_frames = ms_to_frames(sample_rate, jitter.smoothing_ms).max(1);
    let interval_index = (frame_index / interval_frames) as u64;
    let progress = (frame_index % interval_frames) as f32 / interval_frames as f32;
    let seed = jitter.seed.unwrap_or(0);

    AmbisonicsCartesianPosition {
        x: lerp(
            noise_value(seed, 0, interval_index),
            noise_value(seed, 0, interval_index + 1),
            progress,
        ) * jitter.spread.x,
        y: lerp(
            noise_value(seed, 1, interval_index),
            noise_value(seed, 1, interval_index + 1),
            progress,
        ) * jitter.spread.y,
        z: lerp(
            noise_value(seed, 2, interval_index),
            noise_value(seed, 2, interval_index + 1),
            progress,
        ) * jitter.spread.z,
    }
}

fn encode_foa_acn(
    normalization: AmbisonicsNormalization,
    position: AmbisonicsCartesianPosition,
) -> [f32; 4] {
    let radius =
        (position.x * position.x + position.y * position.y + position.z * position.z).sqrt();
    let distance_gain = if radius > 1.0 { 1.0 / radius } else { 1.0 };
    let (x, y, z) = if radius > 1.0e-6 {
        (
            position.x / radius,
            position.y / radius,
            position.z / radius,
        )
    } else {
        (0.0, 0.0, 0.0)
    };

    match normalization {
        AmbisonicsNormalization::Sn3d => [
            distance_gain * SQRT_HALF,
            distance_gain * y,
            distance_gain * z,
            distance_gain * x,
        ],
        AmbisonicsNormalization::N3d => [
            distance_gain,
            distance_gain * SQRT_THREE * y,
            distance_gain * SQRT_THREE * z,
            distance_gain * SQRT_THREE * x,
        ],
    }
}

fn lerp_position(
    start: &AmbisonicsCartesianPosition,
    end: &AmbisonicsCartesianPosition,
    progress: f32,
) -> AmbisonicsCartesianPosition {
    AmbisonicsCartesianPosition {
        x: lerp(start.x, end.x, progress),
        y: lerp(start.y, end.y, progress),
        z: lerp(start.z, end.z, progress),
    }
}

fn catmull_rom_position(
    previous: &AmbisonicsCartesianPosition,
    start: &AmbisonicsCartesianPosition,
    end: &AmbisonicsCartesianPosition,
    following: &AmbisonicsCartesianPosition,
    progress: f32,
    tension: f32,
) -> AmbisonicsCartesianPosition {
    AmbisonicsCartesianPosition {
        x: catmull_rom_component(previous.x, start.x, end.x, following.x, progress, tension),
        y: catmull_rom_component(previous.y, start.y, end.y, following.y, progress, tension),
        z: catmull_rom_component(previous.z, start.z, end.z, following.z, progress, tension),
    }
}

fn catmull_rom_component(
    previous: f32,
    start: f32,
    end: f32,
    following: f32,
    progress: f32,
    tension: f32,
) -> f32 {
    let scaled_tension = 0.5 * (1.0 - tension);
    let m1 = (end - previous) * scaled_tension;
    let m2 = (following - start) * scaled_tension;
    let t2 = progress * progress;
    let t3 = t2 * progress;

    (2.0 * t3 - 3.0 * t2 + 1.0) * start
        + (t3 - 2.0 * t2 + progress) * m1
        + (-2.0 * t3 + 3.0 * t2) * end
        + (t3 - t2) * m2
}

fn lerp(start: f32, end: f32, progress: f32) -> f32 {
    start + (end - start) * progress
}

fn ms_to_frames(sample_rate: u32, duration_ms: u32) -> usize {
    ((sample_rate as u64 * duration_ms as u64) / 1_000) as usize
}

fn frames_to_ms(sample_rate: u32, frame_index: usize) -> f32 {
    frame_index as f32 * 1_000.0 / sample_rate as f32
}

fn noise_value(seed: u64, axis: u64, step: u64) -> f32 {
    let mixed = splitmix64(
        seed ^ axis.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ step.wrapping_mul(0xbf58_476d_1ce4_e5b9),
    );
    let normalized = ((mixed >> 40) as u32) as f32 / ((1_u32 << 24) - 1) as f32;
    normalized * 2.0 - 1.0
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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
    fn render_reconstruction_encodes_foa_ambisonics_output() {
        let plan = RenderPlan {
            mode: RenderMode::AmbisonicsReserved,
            stereo_routing: StereoRouting::DuplicateMono,
            ambisonics: ambisonics_plan_with_positioning(
                r#"{
  "trajectory": [
    {
      "time_ms": 0,
      "position": {
        "x": 1.0,
        "y": 0.0,
        "z": 0.0
      }
    }
  ],
  "jitter": {
    "per_grain": false,
    "spread": {
      "x": 0.0,
      "y": 0.0,
      "z": 0.0
    }
  }
}"#,
            ),
            post_convolution: disabled_post_convolution(),
        };
        let reconstruction = MonoBuffer::new(48_000, vec![1.0]).expect("buffer");

        let rendered = render_reconstruction(&plan, &reconstruction).expect("rendering");

        assert_eq!(rendered.channels, 4);
        let expected = [std::f32::consts::FRAC_1_SQRT_2, 0.0_f32, 0.0_f32, 1.0_f32];
        for (actual, expected) in rendered.samples.iter().zip(expected.into_iter()) {
            assert!((*actual - expected).abs() < 1.0e-6);
        }
    }

    #[test]
    fn render_reconstruction_rejects_higher_order_ambisonics() {
        let mut plan = RenderPlan {
            mode: RenderMode::AmbisonicsReserved,
            stereo_routing: StereoRouting::DuplicateMono,
            ambisonics: ambisonics_plan_with_positioning(
                r#"{
  "trajectory": [
    {
      "time_ms": 0,
      "position": {
        "x": 0.0,
        "y": 1.0,
        "z": 0.0
      }
    }
  ],
  "jitter": {
    "per_grain": false,
    "spread": {
      "x": 0.0,
      "y": 0.0,
      "z": 0.0
    }
  }
}"#,
            ),
            post_convolution: disabled_post_convolution(),
        };
        plan.ambisonics.order = 2;
        let reconstruction = MonoBuffer::new(48_000, vec![0.25]).expect("buffer");

        let error = render_reconstruction(&plan, &reconstruction).expect_err("rendering must fail");

        assert_eq!(
            error,
            "ambisonics rendering currently supports only order = 1"
        );
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

        let mut plan = RenderPlan::from(&config.rendering);
        plan.ambisonics.positioning = Some(
            crate::config::load_ambisonics_positioning_spec(&json_path)
                .expect("positioning should load"),
        );
        assert_eq!(plan.ambisonics.order, 1);
        assert_eq!(plan.ambisonics.channel_ordering.as_str(), "acn");
        assert_eq!(plan.ambisonics.normalization.as_str(), "sn3d");
        assert_eq!(
            plan.ambisonics.positioning_json_path,
            Some(json_path.to_string_lossy().into_owned())
        );

        let reconstruction = MonoBuffer::new(48_000, vec![0.25, -0.5, 0.75]).expect("buffer");
        let rendered = render_reconstruction(&plan, &reconstruction)
            .expect("ambisonics render should succeed");

        assert_eq!(rendered.channels, 4);
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

    fn ambisonics_plan_with_positioning(json: &str) -> AmbisonicsRenderPlan {
        let fixture = TempFixtureDir::new();
        let json_path = fixture.write_text_file("positioning.json", json);
        let mut config = AppConfig::default();
        config.rendering.mode = RenderMode::AmbisonicsReserved;
        config.rendering.ambisonics.positioning_json_path =
            json_path.to_string_lossy().into_owned();
        config
            .validate()
            .expect("ambisonics config should validate");

        let mut plan = AmbisonicsRenderPlan::from(&config.rendering.ambisonics);
        plan.positioning = Some(
            crate::config::load_ambisonics_positioning_spec(&json_path)
                .expect("positioning should load"),
        );
        plan
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
