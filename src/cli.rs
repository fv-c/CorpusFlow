#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliCommand {
    Help,
    Run,
    ShowConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            Some("run") => Ok(Self {
                command: CliCommand::Run,
            }),
            Some("show-config") => Ok(Self {
                command: CliCommand::ShowConfig,
            }),
            Some(other) => Err(format!("unknown command `{other}`\n\n{}", usage())),
        }
    }
}

pub fn usage() -> String {
    [
        "CorpusFlow",
        "",
        "USAGE:",
        "  corpusflow help",
        "  corpusflow run",
        "  corpusflow show-config",
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
        assert_eq!(cli.command, CliCommand::Run);
    }

    #[test]
    fn parses_show_config_command() {
        let cli = ParsedCli::parse(["corpusflow", "show-config"]).expect("cli should parse");
        assert_eq!(cli.command, CliCommand::ShowConfig);
    }

    #[test]
    fn rejects_unknown_command() {
        let error = ParsedCli::parse(["corpusflow", "oops"]).expect_err("cli should fail");
        assert!(error.contains("unknown command `oops`"));
    }
}
