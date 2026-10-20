use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{
    input::{InputSchema, ReadRepresentation},
    layout::FlowcellLayout,
};
use clap::{Parser, Subcommand};

#[derive(Clone, Copy, Debug, serde::Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum FigureFormat {
    Html,
    Svg,
    Png,
    Pdf,
    Ps,
    Jpeg,
    Tiff,
    Bmp,
}

impl FigureFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Pdf => "pdf",
            Self::Ps => "ps",
            Self::Jpeg => "jpeg",
            Self::Tiff => "tiff",
            Self::Bmp => "bmp",
        }
    }

    pub fn check_supported(self) -> anyhow::Result<()> {
        anyhow::ensure!(
            cfg!(feature = "static-figures") || matches!(self, Self::Html | Self::Svg),
            "--format {} requires the static-figures Cargo feature; use an official full build or rebuild with --features static-figures",
            self.extension()
        );
        Ok(())
    }
}

#[derive(Parser)]
#[command(
    name = "ontrunqc",
    version,
    about = "Quality control for Nanopore sequencing summaries"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Analyze plain or gzip sequencing summaries from files or a directory.
    Analyze {
        /// Sequencing-summary files or directories (recursively discovered).
        #[arg(num_args = 0..)]
        input: Vec<PathBuf>,
        /// Additional explicit input paths (files or directories).
        #[arg(short = 'i', long = "input", value_name = "PATH")]
        input_paths: Vec<PathBuf>,
        /// Produce only the combined report.
        #[arg(short = 'c', long, action = clap::ArgAction::Set, num_args = 0..=1, require_equals = true, default_missing_value = "true", default_value = "false", value_parser = logical)]
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
        /// Add provisional R-grid densities (also retains zero-length Q observations).
        #[arg(long)]
        minionqc_density: bool,
        /// Reference ggplot2 histogram/Bin2D layers (one extra guarded input pass).
        #[arg(long)]
        minionqc_plot_bins: bool,
        /// Reference hourly summary counts and read-start cumulative curves (guarded reread and bounded numeric disk sort).
        #[arg(long)]
        minionqc_time_series: bool,
        /// Add provisional raw-read fits (extra streaming pass; R parity unverified).
        #[arg(long)]
        minionqc_trends: bool,
        /// Also emit a MinIONQC-shaped summary.yaml.
        #[arg(long)]
        yaml: bool,
        /// Independent figure format; JSON and standalone HTML are always written.
        #[arg(short = 'f', long, value_enum, default_value = "html")]
        format: FigureFormat,
        /// Compact publication dimensions for independent figures.
        #[arg(short = 's', long, alias = "smallfigures", action = clap::ArgAction::Set, num_args = 0..=1, require_equals = true, default_missing_value = "true", default_value = "false", value_parser = logical)]
        small_figures: bool,
        /// Annotate length/Q histograms with existing exact summary statistics.
        #[arg(short = 'a', long, action = clap::ArgAction::Set, num_args = 0..=1, require_equals = true, default_missing_value = "true", default_value = "false", value_parser = logical)]
        add_stat: bool,
        /// Mux reference hours; 0 disables normally, auto interval in compatibility mode.
        #[arg(short = 'm', long = "muxes", alias = "mux-interval-hours", default_value = "0", value_parser = mux_hours)]
        muxes: f64,
        /// Compare streaming SHA-256 across passes and reread sources before publication.
        #[arg(long)]
        verify_input_content: bool,
        /// Maximum concurrent independent input-group aggregators (default 1).
        #[arg(short = 'p', long, default_value = "1", value_parser = clap::value_parser!(u16).range(1..))]
        processors: u16,
        /// Explicit physical map: PromethION 1–3000 or MinION R9.5 1–512.
        #[arg(long, value_enum)]
        flowcell_layout: Option<FlowcellLayout>,
        /// Independent inclusive mean read Q-score cutoff for MinIONQC comparisons.
        #[arg(short = 'q', long, alias = "qscore_cutoff", default_value = "7", allow_hyphen_values = true, value_parser = qscore_cutoff)]
        qscore_cutoff: f64,
        /// Opt in to historical mux reference defaults and safe implicit output.
        #[arg(long)]
        minionqc_compat: bool,
        /// Report directory (required unless --minionqc-compat; replaces existing reports).
        #[arg(short, long, alias = "outputdirectory", value_name = "DIRECTORY")]
        output: Option<PathBuf>,
    },
}

fn qscore_cutoff(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|q| q.is_finite())
        .ok_or_else(|| "Q-score cutoff must be finite".to_owned())
}

fn mux_hours(value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0 && (v * 3600.0).is_finite())
        .ok_or_else(|| {
            "Mux interval must be finite nonnegative hours (0 uses the selected mode default)"
                .into()
        })
}

fn logical(value: &str) -> Result<bool, String> {
    if value.eq_ignore_ascii_case("true") {
        Ok(true)
    } else if value.eq_ignore_ascii_case("false") {
        Ok(false)
    } else {
        Err("expected TRUE or FALSE (or use the flag alone)".into())
    }
}

/// Accept historical spaced logical values without making a flag consume the
/// next input path. Clap itself accepts flag-only / equals syntax unambiguously.
pub fn arguments(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    let mut args = args.into_iter().peekable();
    let mut normalized = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--" {
            normalized.push(arg);
            normalized.extend(args);
            break;
        }
        if matches!(
            arg.to_str(),
            Some(
                "--smallfigures"
                    | "--small-figures"
                    | "-s"
                    | "--combined-only"
                    | "-c"
                    | "--add-stat"
                    | "-a"
            )
        ) && args
            .peek()
            .and_then(|a| a.to_str())
            .is_some_and(|v| logical(v).is_ok())
        {
            let value = args.next().unwrap();
            let mut option = arg;
            option.push("=");
            option.push(value);
            normalized.push(option);
        } else {
            normalized.push(arg);
        }
    }
    normalized
}

pub fn output_directory(
    inputs: &[PathBuf],
    output: Option<PathBuf>,
    compatible: bool,
) -> anyhow::Result<PathBuf> {
    if let Some(output) = output {
        return Ok(output);
    }
    anyhow::ensure!(
        compatible,
        "--output is required (or explicitly opt in with --minionqc-compat)"
    );
    anyhow::ensure!(
        inputs.len() == 1,
        "implicit compatibility output requires one input file/directory; provide --output for multiple paths"
    );
    let input = &inputs[0];
    let directory = if std::fs::metadata(input)?.is_dir() {
        input.as_path()
    } else {
        input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    };
    Ok(directory.join("ontrunqc_qc"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_arguments_preserve_paths_and_double_dash() {
        for flag in [
            "--smallfigures",
            "--small-figures",
            "-s",
            "--add-stat",
            "-a",
            "--combined-only",
            "-c",
        ] {
            for value in ["TRUE", "FALSE", "true", "false"] {
                let parsed = Cli::try_parse_from(arguments(
                    ["ontrunqc", "analyze", flag, value, "reads.tsv", "-o", "qc"]
                        .map(OsString::from),
                ));
                assert!(parsed.is_ok(), "{flag} {value}");
            }
            assert!(
                Cli::try_parse_from(arguments(
                    ["ontrunqc", "analyze", flag, "reads.tsv", "-o", "qc"].map(OsString::from)
                ))
                .is_ok()
            );
        }
        let raw = ["ontrunqc", "analyze", "--", "--add-stat", "TRUE"].map(OsString::from);
        assert_eq!(arguments(raw.clone()), raw);
    }
}
