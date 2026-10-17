use std::path::PathBuf;

use crate::{
    input::{InputSchema, ReadRepresentation},
    layout::FlowcellLayout,
};
use clap::{Parser, Subcommand};

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum FigureFormat {
    Html,
    Svg,
}

#[derive(Parser)]
#[command(
    name = "poregauge",
    version,
    about = "Quality control for Nanopore sequencing summaries"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Analyze uncompressed sequencing summaries from files or a directory.
    Analyze {
        /// Sequencing-summary files or directories (recursively discovered).
        #[arg(num_args = 0..)]
        input: Vec<PathBuf>,
        /// Additional explicit input paths (files or directories).
        #[arg(short = 'i', long = "input", value_name = "PATH")]
        input_paths: Vec<PathBuf>,
        /// Produce only the combined report.
        #[arg(short = 'c', long)]
        combined_only: bool,
        /// Explicitly treat identity-free inputs as chunks of one source.
        #[arg(long)]
        group_inputs: bool,
        /// Split a mixed-run single file into individual and combined reports.
        #[arg(long)]
        split_runs: bool,
        /// Auto-detect minimal legacy headers, or enforce the modern schema.
        #[arg(long, value_enum, default_value = "auto")]
        input_schema: InputSchema,
        /// Choose one complete read representation; auto prefers the 2D pair.
        #[arg(long, value_enum, default_value = "auto")]
        read_representation: ReadRepresentation,
        /// Also emit a MinIONQC-shaped summary.yaml.
        #[arg(long)]
        yaml: bool,
        /// Figure export: svg writes independent figures; raster/PDF/PS are unsupported.
        #[arg(short = 'f', long, value_enum, default_value = "html")]
        format: FigureFormat,
        /// Compact publication dimensions for SVG figures.
        #[arg(short = 's', long, alias = "smallfigures")]
        small_figures: bool,
        /// Annotate length/Q histograms with existing exact summary statistics.
        #[arg(short = 'a', long)]
        add_stat: bool,
        /// Configured mux reference interval in hours; 0 disables markers.
        #[arg(short = 'm', long = "muxes", alias = "mux-interval-hours", default_value = "0", value_parser = mux_hours)]
        muxes: f64,
        /// Compatibility option: only sequential processing (1) is supported.
        #[arg(short = 'p', long, default_value = "1", value_parser = clap::value_parser!(u16).range(1..=1))]
        processors: u16,
        /// Explicit physical map: PromethION 1–3000 or MinION R9.5 1–512.
        #[arg(long, value_enum)]
        flowcell_layout: Option<FlowcellLayout>,
        /// Independent inclusive mean read Q-score cutoff for MinIONQC comparisons.
        #[arg(short = 'q', long, alias = "qscore_cutoff", default_value = "7", allow_hyphen_values = true, value_parser = qscore_cutoff)]
        qscore_cutoff: f64,
        /// Directory for report.json and report.html (replaces existing reports).
        #[arg(short, long, alias = "outputdirectory", value_name = "DIRECTORY")]
        output: PathBuf,
    },
}

fn qscore_cutoff(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|q| q.is_finite() && *q >= 0.0)
        .ok_or_else(|| "Q-score cutoff must be finite and nonnegative".to_owned())
}

fn mux_hours(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0 && (v * 3600.0).is_finite())
        .ok_or_else(|| "Mux interval must be finite nonnegative hours (0 disables markers)".into())
}
