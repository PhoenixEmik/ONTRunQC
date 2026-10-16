mod cli;
mod input;
mod metrics;
mod record;
mod report;

use std::fs::File;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command};
use input::SummaryReader;
use metrics::Aggregator;

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Analyze { input, output } => {
            let file = File::open(&input)
                .with_context(|| format!("cannot open input {}", input.display()))?;
            let mut reader = SummaryReader::new(file)
                .with_context(|| format!("invalid input {}", input.display()))?;
            let mut metrics = Aggregator::default();
            while let Some(record) = reader.next_record()? {
                metrics
                    .add(&record)
                    .with_context(|| format!("line {}", record.line))?;
            }
            let report = metrics.finish(reader.run_id().map(str::to_owned));
            let path = report::write(&report, &output)?;
            eprintln!("Report written to {}", path.display());
            Ok(())
        }
    }
}
