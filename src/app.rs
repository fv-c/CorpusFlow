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
    let config = AppConfig::default();
    config.validate()?;

    let output = match cli.command {
        CliCommand::Help => usage(),
        CliCommand::Run => run_message(&config),
        CliCommand::ShowConfig => config.summary(),
    };

    Ok(output)
}

fn run_message(config: &AppConfig) -> String {
    let descriptor = baseline_descriptor_spec();

    format!(
        "CorpusFlow scaffold ready: grain={}ms hop={}ms descriptor_dims={} matcher(alpha={}, beta={})",
        config.corpus.grain_size_ms,
        config.corpus.grain_hop_ms,
        descriptor.dimensions,
        config.matching.alpha,
        config.matching.beta
    )
}
