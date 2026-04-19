#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Help,
    Run { config_path: Option<String> },
    ShowConfig,
    ValidateConfig { config_path: String },
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
        match args.as_slice() {
            [] => Ok(Self {
                command: CliCommand::Run { config_path: None },
            }),
            [flag, path] if flag == "--config" => Ok(Self {
                command: CliCommand::Run {
                    config_path: Some(path.clone()),
                },
            }),
            [flag] if flag == "--config" => {
                Err(format!("missing value for `--config`\n\n{}", usage()))
            }
            _ => Err(format!("invalid arguments for `run`\n\n{}", usage())),
        }
    }

    fn parse_show_config(args: Vec<String>) -> Result<Self, String> {
        if args.is_empty() {
            Ok(Self {
                command: CliCommand::ShowConfig,
            })
        } else {
            Err(format!("`show-config` does not accept arguments\n\n{}", usage()))
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
            _ => Err(format!("invalid arguments for `validate-config`\n\n{}", usage())),
        }
    }
}

pub fn usage() -> String {
    [
        "CorpusFlow",
        "",
        "USAGE:",
        "  corpusflow help",
        "  corpusflow run [--config PATH]",
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
        let cli = ParsedCli::parse(["corpusflow", "run"]).expect("cli should parse");
        assert_eq!(cli.command, CliCommand::Run { config_path: None });
    }

    #[test]
    fn parses_run_command_with_config_path() {
        let cli = ParsedCli::parse(["corpusflow", "run", "--config", "release.json"])
            .expect("cli should parse");
        assert_eq!(
            cli.command,
            CliCommand::Run {
                config_path: Some("release.json".to_string())
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
    fn rejects_show_config_arguments() {
        let error = ParsedCli::parse(["corpusflow", "show-config", "extra"])
            .expect_err("cli should fail");
        assert!(error.contains("`show-config` does not accept arguments"));
    }

    #[test]
    fn rejects_missing_validate_config_path() {
        let error =
            ParsedCli::parse(["corpusflow", "validate-config"]).expect_err("cli should fail");
        assert!(error.contains("missing config path for `validate-config`"));
    }
}
