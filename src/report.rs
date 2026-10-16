use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::metrics::{
    ChannelMetrics, EndReasonMetrics, LengthBin, MuxMetrics, OverallMetrics, QualityMetrics,
    TimeBin,
};

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub tool: Tool,
    pub run: Run,
    pub metrics: OverallMetrics,
    pub quality: QualityMetrics,
    pub read_length_distribution: Vec<LengthBin>,
    pub channels: Vec<ChannelMetrics>,
    pub muxes: Vec<MuxMetrics>,
    pub time_bins: Vec<TimeBin>,
    pub end_reasons: Vec<EndReasonMetrics>,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    name: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Run {
    pub run_id: Option<String>,
}

impl Tool {
    pub fn current() -> Self {
        Self {
            name: "PoreGauge",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

pub fn write(report: &Report, directory: &Path) -> Result<PathBuf> {
    fs::create_dir_all(directory)
        .with_context(|| format!("cannot create output directory {}", directory.display()))?;
    let path = directory.join("report.json");
    let file =
        File::create(&path).with_context(|| format!("cannot create report {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, report).context("cannot serialize JSON report")?;
    writer
        .write_all(b"\n")
        .context("cannot write report newline")?;
    writer.flush().context("cannot flush JSON report")?;
    Ok(path)
}
