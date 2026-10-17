mod cli;
mod html;
mod input;
mod layout;
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
        Command::Analyze {
            input,
            output,
            flowcell_layout,
            qscore_cutoff,
        } => {
            let file = File::open(&input)
                .with_context(|| format!("cannot open input {}", input.display()))?;
            let mut reader = SummaryReader::new(file)
                .with_context(|| format!("invalid input {}", input.display()))?;
            let mut metrics = Aggregator::new(flowcell_layout, reader.has_template_duration());
            metrics.set_qscore_cutoff(qscore_cutoff, reader.has_native_filtering());
            while let Some(record) = reader.next_record()? {
                metrics
                    .add(&record)
                    .with_context(|| format!("line {}", record.line))?;
            }
            let report = metrics.finish(reader.run_id().map(str::to_owned))?;
            report::write(&report, &output)?;
            eprintln!("JSON and HTML reports written to {}", output.display());
            Ok(())
        }
    }
}
