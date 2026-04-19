use crate::{
    cli::{CliCommand, ParsedCli, usage},
    config::AppConfig,
    descriptor::baseline_descriptor_spec,
};

pub fn run<I, S>(args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let cli = ParsedCli::parse(args)?;

    let output = match cli.command {
        CliCommand::Help => usage(),
        CliCommand::Run { config_path } => {
            let config = load_config(config_path.as_deref())?;
            run_message(&config)
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

fn run_message(config: &AppConfig) -> String {
    let descriptor = baseline_descriptor_spec();

    format!(
        "CorpusFlow scaffold ready: grain={}ms hop={}ms descriptor_dims={} matcher(alpha={}, beta={}) micro(gain={}, envelope={}) rendering(mode={}, convolution={})",
        config.corpus.grain_size_ms,
        config.corpus.grain_hop_ms,
        descriptor.dimensions,
        config.matching.alpha,
        config.matching.beta,
        config.micro_adaptation.gain.as_str(),
        config.micro_adaptation.envelope.as_str(),
        config.rendering.mode.as_str(),
        if config.rendering.post_convolution.enabled {
            "on"
        } else {
            "off"
        },
    )
}

fn validate_config_message(config_path: &str) -> Result<String, String> {
    let config = AppConfig::from_json_file(config_path)?;
    Ok(format!(
        "config valid: {config_path}\n{}",
        config.summary()
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::run;

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
        ])
        .expect("run should succeed");

        assert!(output.contains("grain=80ms hop=40ms"));
        assert!(output.contains("matcher(alpha=2, beta=0.75)"));
        assert!(output.contains("micro(gain=match-target-rms, envelope=inherit-carrier-rms)"));
        assert!(output.contains("rendering(mode=stereo, convolution=off)"));
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
    }

    impl Drop for TempFixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
