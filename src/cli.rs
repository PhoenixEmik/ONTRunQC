use std::path::PathBuf;

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
        /// Directory for report.json and report.html (replaces existing reports).
        #[arg(short, long, value_name = "DIRECTORY")]
        output: PathBuf,
    },
}
