use std::path::PathBuf;

use crate::layout::FlowcellLayout;
use clap::{Parser, Subcommand};

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
    /// Analyze a single run from an uncompressed sequencing summary TSV.
    Analyze {
        /// Path to the sequencing summary.
        input: PathBuf,
        /// Explicit physical channel map; requires channel IDs 1–3000.
        #[arg(long, value_enum)]
        flowcell_layout: Option<FlowcellLayout>,
        /// Independent inclusive mean read Q-score cutoff for MinIONQC comparisons.
        #[arg(short = 'q', long, default_value = "7", allow_hyphen_values = true, value_parser = qscore_cutoff)]
        qscore_cutoff: f64,
        /// Directory for report.json and report.html (replaces existing reports).
        #[arg(short, long, value_name = "DIRECTORY")]
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
