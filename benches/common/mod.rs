#![allow(dead_code)]

use corpusflow::{
    audio::MonoBuffer,
    config::{OverlapScheduleMode, RenderMode, StereoRouting, WindowKind},
    corpus::CorpusSourceFile,
    descriptor::{DescriptorNormalization, DescriptorVector},
    index::{CorpusGrainEntry, CorpusIndex, CorpusSourceInfo},
    matching::{MatchCost, MatchSequence, MatchStep, MatchingModel},
    rendering::{AmbisonicsRenderPlan, PostConvolutionPlan, RenderPlan},
    synthesis::SynthesisPlan,
    target::{TargetAnalysis, TargetAnalysisFrame},
};
use std::path::PathBuf;

pub const SAMPLE_RATE: u32 = 48_000;
pub const GRAIN_SIZE_FRAMES: usize = 4_800;
pub const GRAIN_HOP_FRAMES: usize = 2_400;
pub const TARGET_HOP_FRAMES: usize = 2_400;

pub fn build_benchmark_frame(frame_size: usize) -> Vec<f32> {
    let mut frame = Vec::with_capacity(frame_size);

    for index in 0..frame_size {
        let time = index as f32 / SAMPLE_RATE as f32;
        let envelope = 0.65 + 0.35 * (2.0 * std::f32::consts::PI * 0.75 * time).sin();
        let sample = envelope
            * (0.6 * (2.0 * std::f32::consts::PI * 220.0 * time).sin()
                + 0.25 * (2.0 * std::f32::consts::PI * 880.0 * time).sin()
                + 0.15 * (2.0 * std::f32::consts::PI * 1760.0 * time).sin());
        frame.push(sample.clamp(-1.0, 1.0));
    }

    frame
}

pub fn build_corpus_sources(grain_count: usize) -> Vec<CorpusSourceFile> {
    let frame_count = required_source_frames(grain_count.max(1));

    vec![CorpusSourceFile {
        path: PathBuf::from("benchmark_source.wav"),
        audio: MonoBuffer::new(SAMPLE_RATE, build_source_samples(frame_count)).expect("source"),
    }]
}

pub fn build_corpus_index(grain_count: usize) -> CorpusIndex {
    let descriptors = (0..grain_count)
        .map(|index| descriptor_for(index, 0))
        .collect::<Vec<_>>();

    CorpusIndex {
        sources: vec![CorpusSourceInfo {
            path: PathBuf::from("benchmark_source.wav"),
            sample_rate: SAMPLE_RATE,
            total_frames: required_source_frames(grain_count.max(1)),
        }],
        grains: (0..grain_count)
            .map(|index| CorpusGrainEntry {
                source_index: 0,
                start_frame: index * GRAIN_HOP_FRAMES,
                len_frames: GRAIN_SIZE_FRAMES,
            })
            .collect(),
        raw_descriptors: descriptors.clone(),
        normalized_descriptors: descriptors,
        normalization: DescriptorNormalization {
            mean: [0.0; 5],
            scale: [1.0; 5],
        },
    }
}

pub fn build_matching_model() -> MatchingModel {
    MatchingModel {
        alpha: 1.0,
        beta: 0.25,
        transition_descriptor_weight: 1.0,
        transition_seek_weight: 0.5,
        source_switch_penalty: 0.25,
    }
}

pub fn build_target_analysis(frame_count: usize) -> TargetAnalysis {
    let frames = (0..frame_count)
        .map(|index| {
            let descriptor = descriptor_for(index, 7);

            TargetAnalysisFrame {
                start_frame: index * TARGET_HOP_FRAMES,
                len_frames: GRAIN_SIZE_FRAMES,
                rms: 0.4 + (index % 17) as f32 / 32.0,
                raw_descriptor: descriptor,
                normalized_descriptor: descriptor,
            }
        })
        .collect::<Vec<_>>();

    let total_frames = if frame_count == 0 {
        0
    } else {
        GRAIN_SIZE_FRAMES + (frame_count - 1) * TARGET_HOP_FRAMES
    };

    TargetAnalysis {
        sample_rate: SAMPLE_RATE,
        original_channels: 1,
        total_frames,
        frame_size_frames: GRAIN_SIZE_FRAMES,
        hop_size_frames: TARGET_HOP_FRAMES,
        frames,
    }
}

pub fn build_match_sequence(step_count: usize, grain_count: usize) -> MatchSequence {
    MatchSequence {
        total_cost: 0.0,
        steps: (0..step_count)
            .map(|target_frame_index| MatchStep {
                target_frame_index,
                selected_grain_index: target_frame_index % grain_count,
                cost: MatchCost {
                    target_distance: 0.0,
                    transition_cost: 0.0,
                    transition_descriptor_distance: 0.0,
                    transition_seek_distance: 0.0,
                    source_switch_cost: 0.0,
                    total_cost: 0.0,
                },
            })
            .collect(),
    }
}

pub fn build_synthesis_plan() -> SynthesisPlan {
    SynthesisPlan {
        window: WindowKind::Hann,
        output_hop_ms: 50,
        overlap_schedule: OverlapScheduleMode::Alternating,
        irregularity_ms: 10,
    }
}

pub fn build_render_plan() -> RenderPlan {
    RenderPlan {
        mode: RenderMode::Mono,
        stereo_routing: StereoRouting::DuplicateMono,
        ambisonics: AmbisonicsRenderPlan {
            positioning_json_path: None,
        },
        post_convolution: PostConvolutionPlan {
            enabled: true,
            impulse_response: vec![0.45, 0.3, 0.15, 0.075, 0.025],
            dry_mix: 0.8,
            wet_mix: 0.6,
            normalize_output: true,
        },
    }
}

pub fn descriptor_for(index: usize, offset: usize) -> DescriptorVector {
    let base = index + offset;

    DescriptorVector::new([
        (base % 17) as f32 / 17.0,
        ((base * 3) % 19) as f32 / 19.0,
        ((base * 5) % 23) as f32 / 23.0,
        ((base * 7) % 29) as f32 / 29.0,
        ((base * 11) % 31) as f32 / 31.0,
    ])
}

fn required_source_frames(grain_count: usize) -> usize {
    (grain_count - 1) * GRAIN_HOP_FRAMES + GRAIN_SIZE_FRAMES
}

fn build_source_samples(frame_count: usize) -> Vec<f32> {
    let mut samples = Vec::with_capacity(frame_count);

    for index in 0..frame_count {
        let time = index as f32 / SAMPLE_RATE as f32;
        let slow_lfo = 0.75 + 0.25 * (2.0 * std::f32::consts::PI * 0.5 * time).sin();
        let sample = slow_lfo
            * (0.7 * (2.0 * std::f32::consts::PI * 220.0 * time).sin()
                + 0.2 * (2.0 * std::f32::consts::PI * 440.0 * time).sin()
                + 0.1 * (2.0 * std::f32::consts::PI * 1760.0 * time).sin());
        samples.push(sample.clamp(-1.0, 1.0));
    }

    samples
}
