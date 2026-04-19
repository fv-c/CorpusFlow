use std::{f32::consts::PI, sync::Arc};

use rustfft::{Fft, FftPlanner, num_complex::Complex};

pub const BASELINE_DESCRIPTOR_DIMENSIONS: usize = 5;
const DESCRIPTOR_EPSILON: f32 = 1.0e-12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorSpec {
    pub dimensions: usize,
    pub feature_names: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DescriptorVector {
    pub values: [f32; BASELINE_DESCRIPTOR_DIMENSIONS],
}

impl DescriptorVector {
    pub const fn new(values: [f32; BASELINE_DESCRIPTOR_DIMENSIONS]) -> Self {
        Self { values }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptorNormalization {
    pub mean: [f32; BASELINE_DESCRIPTOR_DIMENSIONS],
    pub scale: [f32; BASELINE_DESCRIPTOR_DIMENSIONS],
}

impl DescriptorNormalization {
    pub fn fit(descriptors: &[DescriptorVector]) -> Result<Self, String> {
        if descriptors.is_empty() {
            return Err("descriptor normalization requires at least one vector".to_string());
        }

        let mut mean = [0.0; BASELINE_DESCRIPTOR_DIMENSIONS];
        let mut variance = [0.0; BASELINE_DESCRIPTOR_DIMENSIONS];
        let count = descriptors.len() as f32;

        for descriptor in descriptors {
            for (index, value) in descriptor.values.iter().copied().enumerate() {
                mean[index] += value;
            }
        }

        for value in &mut mean {
            *value /= count;
        }

        for descriptor in descriptors {
            for (index, value) in descriptor.values.iter().copied().enumerate() {
                let delta = value - mean[index];
                variance[index] += delta * delta;
            }
        }

        let mut scale = [1.0; BASELINE_DESCRIPTOR_DIMENSIONS];
        for index in 0..BASELINE_DESCRIPTOR_DIMENSIONS {
            let sigma = (variance[index] / count).sqrt();
            if sigma > DESCRIPTOR_EPSILON {
                scale[index] = sigma;
            }
        }

        Ok(Self { mean, scale })
    }

    pub fn normalize(&self, descriptor: DescriptorVector) -> DescriptorVector {
        let mut values = descriptor.values;

        for (index, value) in values.iter_mut().enumerate() {
            *value = (*value - self.mean[index]) / self.scale[index];
        }

        DescriptorVector::new(values)
    }

    pub fn normalize_in_place(&self, descriptors: &mut [DescriptorVector]) {
        for descriptor in descriptors {
            *descriptor = self.normalize(*descriptor);
        }
    }
}

pub struct BaselineDescriptorExtractor {
    sample_rate: u32,
    frame_size: usize,
    positive_bin_count: usize,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
}

impl BaselineDescriptorExtractor {
    pub fn new(sample_rate: u32, frame_size: usize) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("descriptor extractor sample_rate must be > 0".to_string());
        }
        if frame_size == 0 {
            return Err("descriptor extractor frame_size must be > 0".to_string());
        }

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(frame_size);
        let scratch_len = fft.get_inplace_scratch_len();

        Ok(Self {
            sample_rate,
            frame_size,
            positive_bin_count: frame_size / 2 + 1,
            fft,
            window: build_hann_window(frame_size),
            spectrum: vec![Complex::new(0.0, 0.0); frame_size],
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
        })
    }

    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    pub fn extract_frame(&mut self, frame: &[f32]) -> Result<DescriptorVector, String> {
        if frame.len() != self.frame_size {
            return Err(format!(
                "descriptor extractor expected {} samples, found {}",
                self.frame_size,
                frame.len()
            ));
        }

        let log_rms = compute_log_rms(frame);
        let zero_crossing_rate = compute_zero_crossing_rate(frame);

        for (index, complex) in self.spectrum.iter_mut().enumerate() {
            complex.re = frame[index] * self.window[index];
            complex.im = 0.0;
        }

        self.fft
            .process_with_scratch(&mut self.spectrum, &mut self.scratch);

        let (spectral_centroid, spectral_flatness, spectral_rolloff_85) =
            self.compute_spectral_features();

        Ok(DescriptorVector::new([
            log_rms,
            zero_crossing_rate,
            spectral_centroid,
            spectral_flatness,
            spectral_rolloff_85,
        ]))
    }

    fn compute_spectral_features(&self) -> (f32, f32, f32) {
        let bin_hz = self.sample_rate as f32 / self.frame_size as f32;
        let mut power_sum = 0.0;
        let mut weighted_frequency_sum = 0.0;
        let mut log_power_sum = 0.0;

        for bin in 0..self.positive_bin_count {
            let power = self.spectrum[bin].norm_sqr();
            let stabilized_power = power + DESCRIPTOR_EPSILON;
            let frequency = bin as f32 * bin_hz;

            power_sum += power;
            weighted_frequency_sum += frequency * power;
            log_power_sum += stabilized_power.ln();
        }

        if power_sum <= DESCRIPTOR_EPSILON {
            return (0.0, 0.0, 0.0);
        }

        let arithmetic_mean = power_sum / self.positive_bin_count as f32;
        let geometric_mean = (log_power_sum / self.positive_bin_count as f32).exp();
        let centroid = weighted_frequency_sum / power_sum;
        let rolloff = compute_rolloff_frequency(
            &self.spectrum[..self.positive_bin_count],
            bin_hz,
            0.85,
            power_sum,
        );

        (centroid, geometric_mean / arithmetic_mean, rolloff)
    }
}

pub fn baseline_descriptor_spec() -> DescriptorSpec {
    const FEATURES: &[&str] = &[
        "log_rms",
        "zero_crossing_rate",
        "spectral_centroid",
        "spectral_flatness",
        "spectral_rolloff_85",
    ];

    DescriptorSpec {
        dimensions: FEATURES.len(),
        feature_names: FEATURES,
    }
}

fn compute_log_rms(frame: &[f32]) -> f32 {
    let mean_square = frame.iter().map(|sample| sample * sample).sum::<f32>() / frame.len() as f32;
    mean_square.sqrt().max(DESCRIPTOR_EPSILON).ln()
}

fn compute_zero_crossing_rate(frame: &[f32]) -> f32 {
    if frame.len() < 2 {
        return 0.0;
    }

    let crossings = frame
        .windows(2)
        .filter(|pair| sign_bucket(pair[0]) != sign_bucket(pair[1]))
        .count();

    crossings as f32 / (frame.len() - 1) as f32
}

fn compute_rolloff_frequency(
    spectrum: &[Complex<f32>],
    bin_hz: f32,
    threshold_ratio: f32,
    total_power: f32,
) -> f32 {
    let threshold = total_power * threshold_ratio;
    let mut cumulative_power = 0.0;

    for (bin, complex) in spectrum.iter().enumerate() {
        cumulative_power += complex.norm_sqr();
        if cumulative_power >= threshold {
            return bin as f32 * bin_hz;
        }
    }

    (spectrum.len().saturating_sub(1)) as f32 * bin_hz
}

fn build_hann_window(frame_size: usize) -> Vec<f32> {
    if frame_size == 1 {
        return vec![1.0];
    }

    let scale = 2.0 * PI / (frame_size - 1) as f32;
    let mut window = Vec::with_capacity(frame_size);

    for index in 0..frame_size {
        window.push(0.5 - 0.5 * (scale * index as f32).cos());
    }

    window
}

fn sign_bucket(value: f32) -> bool {
    value >= 0.0
}

#[cfg(test)]
mod tests {
    use super::{
        BASELINE_DESCRIPTOR_DIMENSIONS, BaselineDescriptorExtractor, DescriptorNormalization,
        DescriptorVector, baseline_descriptor_spec,
    };

    #[test]
    fn baseline_descriptor_spec_matches_feature_count() {
        let spec = baseline_descriptor_spec();

        assert_eq!(spec.dimensions, BASELINE_DESCRIPTOR_DIMENSIONS);
        assert_eq!(spec.feature_names[0], "log_rms");
        assert_eq!(spec.feature_names[4], "spectral_rolloff_85");
    }

    #[test]
    fn descriptor_extractor_rejects_frame_size_mismatch() {
        let mut extractor = BaselineDescriptorExtractor::new(48_000, 8).expect("extractor");
        let error = extractor
            .extract_frame(&[0.0; 4])
            .expect_err("mismatched frame should fail");

        assert_eq!(error, "descriptor extractor expected 8 samples, found 4");
    }

    #[test]
    fn descriptor_extractor_returns_finite_values_for_silence() {
        let mut extractor = BaselineDescriptorExtractor::new(48_000, 16).expect("extractor");
        let descriptor = extractor
            .extract_frame(&[0.0; 16])
            .expect("descriptor should extract");

        assert!(descriptor.values.iter().all(|value| value.is_finite()));
        assert_eq!(descriptor.values[1], 0.0);
        assert_eq!(descriptor.values[2], 0.0);
        assert_eq!(descriptor.values[3], 0.0);
        assert_eq!(descriptor.values[4], 0.0);
    }

    #[test]
    fn higher_frequency_tone_has_higher_centroid_and_rolloff() {
        let sample_rate = 8_000;
        let frame_size = 256;
        let mut extractor =
            BaselineDescriptorExtractor::new(sample_rate, frame_size).expect("extractor");
        let low = sine_frame(sample_rate, frame_size, 220.0);
        let high = sine_frame(sample_rate, frame_size, 1_760.0);

        let low_descriptor = extractor.extract_frame(&low).expect("low descriptor");
        let high_descriptor = extractor.extract_frame(&high).expect("high descriptor");

        assert!(high_descriptor.values[2] > low_descriptor.values[2]);
        assert!(high_descriptor.values[4] > low_descriptor.values[4]);
    }

    #[test]
    fn descriptor_normalization_centers_and_scales_vectors() {
        let descriptors = [
            DescriptorVector::new([1.0, 0.0, 10.0, 5.0, 100.0]),
            DescriptorVector::new([3.0, 2.0, 14.0, 5.0, 140.0]),
        ];

        let normalization =
            DescriptorNormalization::fit(&descriptors).expect("normalization should fit");
        let mut normalized = descriptors;
        normalization.normalize_in_place(&mut normalized);

        assert_approx_eq(normalized[0].values[0], -1.0);
        assert_approx_eq(normalized[1].values[0], 1.0);
        assert_approx_eq(normalized[0].values[1], -1.0);
        assert_approx_eq(normalized[1].values[1], 1.0);
        assert_approx_eq(normalized[0].values[3], 0.0);
        assert_approx_eq(normalized[1].values[3], 0.0);
    }

    #[test]
    fn descriptor_normalization_requires_non_empty_input() {
        let error =
            DescriptorNormalization::fit(&[]).expect_err("empty normalization input must fail");

        assert_eq!(
            error,
            "descriptor normalization requires at least one vector"
        );
    }

    fn sine_frame(sample_rate: u32, frame_size: usize, frequency_hz: f32) -> Vec<f32> {
        (0..frame_size)
            .map(|index| {
                let time = index as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * frequency_hz * time).sin()
            })
            .collect()
    }

    fn assert_approx_eq(left: f32, right: f32) {
        assert!((left - right).abs() < 1.0e-4, "{left} != {right}");
    }
}
