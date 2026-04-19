#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Help,
    Run {
        config_path: Option<String>,
        output_path: String,
    },
    ShowConfig,
    ValidateConfig {
        config_path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCli {
    pub command: CliCommand,
}

impl ParsedCli {
    pub fn parse<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let _binary = args.next();

        match args.next().as_deref() {
            None | Some("help") | Some("--help") | Some("-h") => Ok(Self {
                command: CliCommand::Help,
            }),
            Some("run") => Self::parse_run(args.collect()),
            Some("show-config") => Self::parse_show_config(args.collect()),
            Some("validate-config") => Self::parse_validate_config(args.collect()),
            Some(other) => Err(format!("unknown command `{other}`\n\n{}", usage())),
        }
    }

    fn parse_run(args: Vec<String>) -> Result<Self, String> {
        let mut config_path = None;
        let mut output_path = None;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--config" => {
                    let Some(path) = args.get(index + 1) else {
                        return Err(format!("missing value for `--config`\n\n{}", usage()));
                    };
                    config_path = Some(path.clone());
                    index += 2;
                }
                "--output" => {
                    let Some(path) = args.get(index + 1) else {
                        return Err(format!("missing value for `--output`\n\n{}", usage()));
                    };
                    output_path = Some(path.clone());
                    index += 2;
                }
                _ => return Err(format!("invalid arguments for `run`\n\n{}", usage())),
            }
        }

        let Some(output_path) = output_path else {
            return Err(format!(
                "missing required `--output PATH` for `run`\n\n{}",
                usage()
            ));
        };

        Ok(Self {
            command: CliCommand::Run {
                config_path,
                output_path,
            },
        })
    }

    fn parse_show_config(args: Vec<String>) -> Result<Self, String> {
        if args.is_empty() {
            Ok(Self {
                command: CliCommand::ShowConfig,
            })
        } else {
            Err(format!(
                "`show-config` does not accept arguments\n\n{}",
                usage()
            ))
        }
    }

    fn parse_validate_config(args: Vec<String>) -> Result<Self, String> {
        match args.as_slice() {
            [path] => Ok(Self {
                command: CliCommand::ValidateConfig {
                    config_path: path.clone(),
                },
            }),
            [] => Err(format!(
                "missing config path for `validate-config`\n\n{}",
                usage()
            )),
            _ => Err(format!(
                "invalid arguments for `validate-config`\n\n{}",
                usage()
            )),
        }
    }
}

pub fn usage() -> String {
    [
        "CorpusFlow",
        "",
        "USAGE:",
        "  corpusflow help",
        "  corpusflow run [--config PATH] --output PATH",
        "  corpusflow show-config",
        "  corpusflow validate-config PATH",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{CliCommand, ParsedCli};

    #[test]
    fn parses_help_when_no_command_is_present() {
        let cli = ParsedCli::parse(["corpusflow"]).expect("cli should parse");
        assert_eq!(cli.command, CliCommand::Help);
    }

    #[test]
    fn parses_run_command() {
        let cli = ParsedCli::parse(["corpusflow", "run", "--output", "out.wav"])
            .expect("cli should parse");
        assert_eq!(
            cli.command,
            CliCommand::Run {
                config_path: None,
                output_path: "out.wav".to_string(),
            }
        );
    }

    #[test]
    fn parses_run_command_with_config_path() {
        let cli = ParsedCli::parse([
            "corpusflow",
            "run",
            "--config",
            "release.json",
            "--output",
            "render.wav",
        ])
        .expect("cli should parse");
        assert_eq!(
            cli.command,
            CliCommand::Run {
                config_path: Some("release.json".to_string()),
                output_path: "render.wav".to_string(),
            }
        );
    }

    #[test]
    fn parses_run_command_when_output_precedes_config() {
        let cli = ParsedCli::parse([
            "corpusflow",
            "run",
            "--output",
            "render.wav",
            "--config",
            "release.json",
        ])
        .expect("cli should parse");
        assert_eq!(
            cli.command,
            CliCommand::Run {
                config_path: Some("release.json".to_string()),
                output_path: "render.wav".to_string(),
            }
        );
    }

    #[test]
    fn parses_show_config_command() {
        let cli = ParsedCli::parse(["corpusflow", "show-config"]).expect("cli should parse");
        assert_eq!(cli.command, CliCommand::ShowConfig);
    }

    #[test]
    fn parses_validate_config_command() {
        let cli = ParsedCli::parse(["corpusflow", "validate-config", "release.json"])
            .expect("cli should parse");
        assert_eq!(
            cli.command,
            CliCommand::ValidateConfig {
                config_path: "release.json".to_string()
            }
        );
    }

    #[test]
    fn rejects_unknown_command() {
        let error = ParsedCli::parse(["corpusflow", "oops"]).expect_err("cli should fail");
        assert!(error.contains("unknown command `oops`"));
    }

    #[test]
    fn rejects_missing_run_config_value() {
        let error =
            ParsedCli::parse(["corpusflow", "run", "--config"]).expect_err("cli should fail");
        assert!(error.contains("missing value for `--config`"));
    }

    #[test]
    fn rejects_missing_run_output_value() {
        let error =
            ParsedCli::parse(["corpusflow", "run", "--output"]).expect_err("cli should fail");
        assert!(error.contains("missing value for `--output`"));
    }

    #[test]
    fn rejects_missing_run_output_flag() {
        let error = ParsedCli::parse(["corpusflow", "run"]).expect_err("cli should fail");
        assert!(error.contains("missing required `--output PATH`"));
    }

    #[test]
    fn rejects_show_config_arguments() {
        let error =
            ParsedCli::parse(["corpusflow", "show-config", "extra"]).expect_err("cli should fail");
        assert!(error.contains("`show-config` does not accept arguments"));
    }

    #[test]
    fn rejects_missing_validate_config_path() {
        let error =
            ParsedCli::parse(["corpusflow", "validate-config"]).expect_err("cli should fail");
        assert!(error.contains("missing config path for `validate-config`"));
    }
}
