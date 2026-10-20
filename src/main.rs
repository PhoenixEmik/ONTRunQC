mod batch;
mod cli;
mod density;
mod export;
#[cfg(feature = "static-figures")]
mod figure_render;
mod html;
mod input;
mod integrity;
mod layout;
mod metrics;
mod minion;
mod plot_bins;
mod record;
mod report;
mod temporal;
mod trend_knots;
mod trends;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() -> std::process::ExitCode {
    match run(Cli::parse_from(cli::arguments(std::env::args_os()))) {
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
            minionqc_compat,
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
            minionqc_plot_bins,
            minionqc_time_series,
            verify_input_content,
        } => {
            format.check_supported()?;
            input.extend(input_paths);
            let implicit_output = output.is_none();
            let output = cli::output_directory(&input, output, minionqc_compat)?;
            if implicit_output {
                std::fs::create_dir(&output).map_err(|e| anyhow::anyhow!("cannot reserve implicit output {}: {e}; choose an unused directory with --output", output.display()))?;
            }
            let result = batch::analyze(
                &input,
                &output,
                &batch::AnalysisOptions {
                    processors: usize::from(processors),
                    verify_input_content,
                    minionqc_compat,
                    layout: flowcell_layout,
                    cutoff: qscore_cutoff,
                    minionqc_trends,
                    minionqc_density,
                    minionqc_plot_bins,
                    minionqc_time_series,
                    combined_only,
                    group_inputs,
                    split_runs,
                    input_schema,
                    read_representation,
                    presentation: report::PresentationOptions {
                        write_yaml: yaml,
                        minionqc_compatibility: minionqc_compat.then_some(true),
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
            );
            if result.is_err() && implicit_output {
                // Remove only an empty reservation. Never delete completed
                // per-run reports or claim collection-wide rollback.
                let _ = std::fs::remove_dir(&output);
            }
            result?;
            eprintln!("Reports written to {}", output.display());
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "../tests/reference/probe.rs"]
mod reference_probe;
