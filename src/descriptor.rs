#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorSpec {
    pub dimensions: usize,
    pub feature_names: &'static [&'static str],
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
