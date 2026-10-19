mod batch;
mod cli;
mod density;
mod export;
#[cfg(feature = "static-figures")]
mod figure_render;
mod html;
mod input;
mod layout;
mod metrics;
mod minion;
mod record;
mod report;
mod trend_knots;
mod trends;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

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
            mut input,
            input_paths,
            output,
            flowcell_layout,
            qscore_cutoff,
            combined_only,
            group_inputs,
            split_runs,
            input_schema,
            read_representation,
            yaml,
            format,
            small_figures,
            add_stat,
            muxes,
            processors,
            minionqc_trends,
            minionqc_density,
        } => {
            format.check_supported()?;
            input.extend(input_paths);
            batch::analyze(
                &input,
                &output,
                &batch::AnalysisOptions {
                    processors: usize::from(processors),
                    layout: flowcell_layout,
                    cutoff: qscore_cutoff,
                    minionqc_trends,
                    minionqc_density,
                    combined_only,
                    group_inputs,
                    split_runs,
                    input_schema,
                    read_representation,
                    presentation: report::PresentationOptions {
                        write_yaml: yaml,
                        export_svg: matches!(format, cli::FigureFormat::Svg),
                        export_format: (!matches!(
                            format,
                            cli::FigureFormat::Html | cli::FigureFormat::Svg
                        ))
                        .then_some(format),
                        small_figures,
                        add_stat,
                        mux_interval_hours: (muxes > 0.0).then_some(muxes),
                        mux_markers_seconds: Vec::new(),
                    },
                },
            )?;
            eprintln!("Reports written to {}", output.display());
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "../tests/reference/probe.rs"]
mod reference_probe;
