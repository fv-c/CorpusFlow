use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::Path,
};

use crate::{
    cli::{CliCommand, ParsedCli, usage},
    config::AppConfig,
    corpus::CorpusPlan,
    index::CorpusIndex,
    matching::{MatchingModel, greedy_match},
    micro_adaptation::MicroAdaptationPlan,
    rendering::{RenderPlan, render_reconstruction, write_output_wav},
    synthesis::SynthesisPlan,
    target::{TargetInput, TargetPlan},
};

pub fn run<I, S>(args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let stderr = io::stderr();
    let progress_mode = if stderr.is_terminal() {
        ProgressMode::Interactive
    } else {
        ProgressMode::Stream
    };
    let mut progress = stderr.lock();
    run_with_progress(args, &mut progress, progress_mode)
}

fn run_with_progress<I, S, W>(
    args: I,
    progress: &mut W,
    progress_mode: ProgressMode,
) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
    W: Write,
{
    let cli = ParsedCli::parse(args)?;

    let output = match cli.command {
        CliCommand::Help => usage(),
        CliCommand::Run {
            config_path,
            output_path,
        } => {
            let config = load_config(config_path.as_deref())?;
            run_pipeline(&config, &output_path, progress, progress_mode)?
        }
        CliCommand::ShowConfig => AppConfig::default().to_pretty_json()?,
        CliCommand::ValidateConfig { config_path } => validate_config_message(&config_path)?,
    };

    Ok(output)
}

fn load_config(config_path: Option<&str>) -> Result<AppConfig, String> {
    let config = match config_path {
        Some(path) => AppConfig::from_json_file(path)?,
        None => {
            let config = AppConfig::default();
            config.validate()?;
            config
        }
    };

    Ok(config)
}

fn run_pipeline<W>(
    config: &AppConfig,
    output_path: &str,
    progress: &mut W,
    progress_mode: ProgressMode,
) -> Result<String, String>
where
    W: Write,
{
    if output_path.trim().is_empty() {
        return Err("run output path must not be empty".to_string());
    }
    if config.corpus.root.trim().is_empty() {
        return Err("run requires corpus.root to be set".to_string());
    }
    if config.target.path.trim().is_empty() {
        return Err("run requires target.path to be set".to_string());
    }

    let mut progress_bar = PipelineProgress::new(progress, 7, progress_mode);
    progress_bar.emit("starting pipeline");

    let corpus_plan = CorpusPlan::from_config(&config.corpus);
    let corpus_sources = corpus_plan.load_sources(&config.corpus.root)?;
    progress_bar.advance("corpus loaded");
    let corpus_segmentations = corpus_plan.segment_sources(&corpus_sources)?;
    progress_bar.advance("corpus segmented");
    let corpus_index = CorpusIndex::build(&corpus_sources, &corpus_segmentations)?;
    progress_bar.advance("corpus indexed");

    let target_plan = TargetPlan::from(&config.target);
    let target_input = TargetInput::load(&config.target)?;
    let target_analysis = target_plan.analyze_against_corpus(
        &corpus_plan,
        &target_input,
        &corpus_index.normalization,
    )?;
    progress_bar.advance("target analyzed");

    let matching_model = MatchingModel::from(&config.matching);
    let match_sequence = greedy_match(&matching_model, &corpus_index, &target_analysis)?;
    progress_bar.advance("matching complete");

    let synthesis_plan = SynthesisPlan::from(&config.synthesis);
    let micro_adaptation = MicroAdaptationPlan::from(&config.micro_adaptation);
    let synthesis = synthesis_plan.synthesize_with_micro_adaptation(
        &corpus_sources,
        &corpus_index,
        &match_sequence,
        &micro_adaptation,
        &target_analysis,
    )?;
    progress_bar.advance("synthesis complete");

    let render_plan = RenderPlan::from(&config.rendering);
    let rendered = render_reconstruction(&render_plan, &synthesis.audio)?;
    prepare_output_parent(output_path)?;
    write_output_wav(output_path, config.rendering.mode, &rendered)?;
    progress_bar.advance("output written");

    Ok(format!(
        "render complete: output={} corpus_sources={} corpus_grains={} target_frames={} matched_steps={} rendered_channels={} rendered_frames={}",
        output_path,
        corpus_sources.len(),
        corpus_index.len(),
        target_analysis.frames.len(),
        match_sequence.steps.len(),
        rendered.channels,
        rendered.frame_count(),
    ))
}

fn prepare_output_parent(output_path: &str) -> Result<(), String> {
    let path = Path::new(output_path);
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create output directory `{}`: {error}",
            parent.display()
        )
    })
}

fn validate_config_message(config_path: &str) -> Result<String, String> {
    let config = AppConfig::from_json_file(config_path)?;
    Ok(format!("config valid: {config_path}\n{}", config.summary()))
}

struct PipelineProgress<'a, W: Write> {
    writer: &'a mut W,
    total_steps: usize,
    completed_steps: usize,
    mode: ProgressMode,
    needs_newline: bool,
}

impl<'a, W> PipelineProgress<'a, W>
where
    W: Write,
{
    const BAR_WIDTH: usize = 24;

    fn new(writer: &'a mut W, total_steps: usize, mode: ProgressMode) -> Self {
        Self {
            writer,
            total_steps,
            completed_steps: 0,
            mode,
            needs_newline: false,
        }
    }

    fn advance(&mut self, label: &str) {
        self.completed_steps = (self.completed_steps + 1).min(self.total_steps);
        self.emit(label);
    }

    fn emit(&mut self, label: &str) {
        let line = self.render_line(label);

        match self.mode {
            ProgressMode::Interactive => {
                let _ = write!(self.writer, "\r\x1b[2K{line}");
                self.needs_newline = true;
                if self.completed_steps == self.total_steps {
                    let _ = writeln!(self.writer);
                    self.needs_newline = false;
                }
            }
            ProgressMode::Stream => {
                let _ = writeln!(self.writer, "{line}");
            }
        }
        let _ = self.writer.flush();
    }

    fn render_line(&self, label: &str) -> String {
        let percent = if self.total_steps == 0 {
            100
        } else {
            self.completed_steps * 100 / self.total_steps
        };
        let bar = self.render_bar();

        format!(
            "CorpusFlow {:>3}% [{}] {}/{} {}",
            percent, bar, self.completed_steps, self.total_steps, label
        )
    }

    fn render_bar(&self) -> String {
        if self.total_steps == 0 {
            return "=".repeat(Self::BAR_WIDTH);
        }

        if self.completed_steps >= self.total_steps {
            return "=".repeat(Self::BAR_WIDTH);
        }

        let filled = self.completed_steps * Self::BAR_WIDTH / self.total_steps;
        let mut bar = String::with_capacity(Self::BAR_WIDTH);
        bar.push_str(&"=".repeat(filled));
        bar.push('>');
        let trailing = Self::BAR_WIDTH.saturating_sub(filled + 1);
        bar.push_str(&"-".repeat(trailing));
        bar
    }
}

impl<W> Drop for PipelineProgress<'_, W>
where
    W: Write,
{
    fn drop(&mut self) {
        if self.needs_newline {
            let _ = writeln!(self.writer);
            let _ = self.writer.flush();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgressMode {
    Interactive,
    Stream,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{ProgressMode, run, run_with_progress};
    use crate::audio::read_wav;
    use hound::{SampleFormat, WavSpec, WavWriter};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn show_config_returns_pretty_printed_default_json() {
        let output = run(["corpusflow", "show-config"]).expect("show-config should succeed");

        assert!(output.contains("\"grain_size_ms\": 100"));
        assert!(output.contains("\"window\": \"hann\""));
        assert!(output.contains("\"mode\": \"mono\""));
    }

    #[test]
    fn validate_config_reads_and_summarizes_json_file() {
        let fixture = TempFixtureDir::new();
        let path = fixture.write_text_file(
            "release.json",
            r#"{
  "corpus": {
    "root": "fixtures/corpus",
    "grain_size_ms": 120,
    "grain_hop_ms": 60,
    "mono_only": true
  },
  "target": {
    "path": "fixtures/target.wav",
    "frame_size_ms": 120,
    "hop_size_ms": 60
  },
  "matching": {
    "alpha": 1.25,
    "beta": 0.5,
    "transition_descriptor_weight": 1.0,
    "transition_seek_weight": 0.5,
    "source_switch_penalty": 0.25
  },
  "micro_adaptation": {
    "gain": "off",
    "envelope": "off"
  },
  "synthesis": {
    "window": "hann",
    "output_hop_ms": 60,
    "overlap_schedule": "fixed",
    "irregularity_ms": 0
  },
  "rendering": {
    "mode": "mono",
    "stereo_routing": "duplicate-mono",
    "post_convolution": {
      "enabled": false,
      "impulse_response": [],
      "dry_mix": 1.0,
      "wet_mix": 1.0,
      "normalize_output": true
    },
    "ambisonics": {
      "positioning_json_path": ""
    }
  }
}"#,
        );

        let output = run([
            "corpusflow",
            "validate-config",
            path.to_string_lossy().as_ref(),
        ])
        .expect("validate-config should succeed");

        assert!(output.contains("config valid:"));
        assert!(output.contains("grain=120ms hop=60ms"));
        assert!(output.contains("matching(alpha=1.25, beta=0.5)"));
    }

    #[test]
    fn run_uses_external_config_when_provided() {
        let fixture = TempFixtureDir::new();
        let path = fixture.write_text_file(
            "release.json",
            r#"{
  "corpus": {
    "root": "",
    "grain_size_ms": 80,
    "grain_hop_ms": 40,
    "mono_only": true
  },
  "target": {
    "path": "",
    "frame_size_ms": 80,
    "hop_size_ms": 40
  },
  "matching": {
    "alpha": 2.0,
    "beta": 0.75,
    "transition_descriptor_weight": 1.0,
    "transition_seek_weight": 0.5,
    "source_switch_penalty": 0.25
  },
  "micro_adaptation": {
    "gain": "match-target-rms",
    "envelope": "inherit-carrier-rms"
  },
  "synthesis": {
    "window": "hann",
    "output_hop_ms": 40,
    "overlap_schedule": "fixed",
    "irregularity_ms": 0
  },
  "rendering": {
    "mode": "stereo",
    "stereo_routing": "duplicate-mono",
    "post_convolution": {
      "enabled": false,
      "impulse_response": [],
      "dry_mix": 1.0,
      "wet_mix": 1.0,
      "normalize_output": true
    },
    "ambisonics": {
      "positioning_json_path": ""
    }
  }
}"#,
        );

        let output = run([
            "corpusflow",
            "run",
            "--config",
            path.to_string_lossy().as_ref(),
            "--output",
            fixture.path().join("render.wav").to_string_lossy().as_ref(),
        ])
        .expect_err("run should fail until audio inputs exist");

        assert!(output.contains("run requires corpus.root to be set"));
    }

    #[test]
    fn run_executes_end_to_end_and_writes_output_wav() {
        let fixture = TempFixtureDir::new();
        fixture.create_dir("corpus");
        fixture.write_pcm16_wav(
            "corpus/source.wav",
            1,
            &[4_000, -4_000, 8_000, -8_000, 4_000, -4_000, 8_000, -8_000],
        );
        let target_path = fixture.write_pcm16_wav(
            "target.wav",
            1,
            &[
                6_000, -6_000, 10_000, -10_000, 6_000, -6_000, 10_000, -10_000,
            ],
        );
        let output_path = fixture.path().join("renders/out.wav");
        let config_path = fixture.write_text_file(
            "release.json",
            &format!(
                r#"{{
  "corpus": {{
    "root": "{}",
    "grain_size_ms": 1,
    "grain_hop_ms": 1,
    "mono_only": true
  }},
  "target": {{
    "path": "{}",
    "frame_size_ms": 1,
    "hop_size_ms": 1
  }},
  "matching": {{
    "alpha": 1.0,
    "beta": 0.25,
    "transition_descriptor_weight": 1.0,
    "transition_seek_weight": 0.5,
    "source_switch_penalty": 0.25
  }},
  "micro_adaptation": {{
    "gain": "match-target-rms",
    "envelope": "inherit-carrier-rms"
  }},
  "synthesis": {{
    "window": "hann",
    "output_hop_ms": 1,
    "overlap_schedule": "fixed",
    "irregularity_ms": 0
  }},
  "rendering": {{
    "mode": "stereo",
    "stereo_routing": "duplicate-mono",
    "post_convolution": {{
      "enabled": false,
      "impulse_response": [],
      "dry_mix": 1.0,
      "wet_mix": 1.0,
      "normalize_output": true
    }},
    "ambisonics": {{
      "positioning_json_path": ""
    }}
  }}
}}"#,
                fixture.path().join("corpus").display(),
                target_path.display()
            ),
        );

        let mut progress = Vec::new();
        let output = run_with_progress(
            [
                "corpusflow",
                "run",
                "--config",
                config_path.to_string_lossy().as_ref(),
                "--output",
                output_path.to_string_lossy().as_ref(),
            ],
            &mut progress,
            ProgressMode::Stream,
        )
        .expect("run should succeed");
        let progress = String::from_utf8(progress).expect("progress output should be utf-8");

        let rendered = read_wav(&output_path).expect("rendered WAV should load");

        assert!(output.contains("render complete:"));
        assert!(output.contains("corpus_sources=1"));
        assert!(output.contains("matched_steps=8"));
        assert!(
            progress.contains("CorpusFlow   0% [>-----------------------] 0/7 starting pipeline")
        );
        assert!(progress.contains("CorpusFlow 100% [========================] 7/7 output written"));
        assert!(progress.contains("4/7 target analyzed"));
        assert_eq!(rendered.channels, 2);
        assert_eq!(rendered.frame_count(), 8);
        assert!(rendered.samples.iter().any(|sample| sample.abs() > 0.0));
    }

    #[test]
    fn interactive_progress_rewrites_the_same_line() {
        let mut output = Vec::new();
        let mut progress = super::PipelineProgress::new(&mut output, 2, ProgressMode::Interactive);

        progress.emit("starting pipeline");
        progress.advance("corpus loaded");
        progress.advance("output written");
        drop(progress);

        let output = String::from_utf8(output).expect("progress output should be utf-8");

        assert!(
            output.contains(
                "\r\x1b[2KCorpusFlow   0% [>-----------------------] 0/2 starting pipeline"
            )
        );
        assert!(
            output
                .contains("\r\x1b[2KCorpusFlow  50% [============>-----------] 1/2 corpus loaded")
        );
        assert!(
            output.contains(
                "\r\x1b[2KCorpusFlow 100% [========================] 2/2 output written\n"
            )
        );
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
                "corpusflow-app-{}-{}-{}",
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

        fn path(&self) -> &Path {
            &self.path
        }

        fn create_dir(&self, relative: &str) {
            fs::create_dir_all(self.path.join(relative)).expect("fixture dir should be created");
        }

        fn write_pcm16_wav(&self, relative: &str, channels: u16, samples: &[i16]) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture parent dir should be created");
            }

            let spec = WavSpec {
                channels,
                sample_rate: 1_000,
                bits_per_sample: 16,
                sample_format: SampleFormat::Int,
            };
            let mut writer = WavWriter::create(&path, spec).expect("fixture wav should be created");

            for &sample in samples {
                writer
                    .write_sample(sample)
                    .expect("fixture sample should be written");
            }

            writer.finalize().expect("fixture wav should finalize");
            path
        }
    }

    impl Drop for TempFixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
