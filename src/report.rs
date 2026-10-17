use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

use crate::metrics::{
    ChannelMetrics, ComparisonMetrics, Diagnostics, EndReasonMetrics, LengthBin, MuxMetrics,
    OverallMetrics, QualityMetrics, TimeBin,
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
    pub diagnostics: Diagnostics,
    pub comparison: ComparisonMetrics,
    pub distributions: crate::metrics::DistributionEstimates,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<Collection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceMetadata {
    pub source_id: String,
    pub input_files: usize,
    pub grouping: &'static str,
    pub flow_cell_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Collection {
    pub unit: &'static str,
    pub runs: Vec<RunComparison>,
}

#[derive(Debug, Serialize)]
pub struct RunComparison {
    pub source: SourceMetadata,
    pub run_id: Option<String>,
    pub metrics: OverallMetrics,
    pub quality: QualityMetrics,
    pub read_length_distribution: Vec<LengthBin>,
    pub qscore_selected: crate::metrics::PassedReadStatistics,
    pub selected_length_histogram: Vec<LengthBin>,
    pub selected_quality_histogram: Vec<crate::metrics::QualityBin>,
    pub time_bins: Vec<TimeBin>,
    pub selected_time_bins: Vec<crate::metrics::SelectedTimeBin>,
    pub yield_by_minimum_length: crate::metrics::YieldByLength,
    pub distributions: crate::metrics::DistributionEstimates,
}

impl RunComparison {
    pub fn from_report(report: &Report) -> Self {
        Self {
            source: report.source.clone().expect("collection source metadata"),
            run_id: report.run.run_id.clone(),
            metrics: report.metrics.clone(),
            quality: report.quality.clone(),
            read_length_distribution: report.read_length_distribution.clone(),
            qscore_selected: report.comparison.qscore_selected.clone(),
            selected_length_histogram: report.comparison.length_histogram.clone(),
            selected_quality_histogram: report.comparison.quality_histogram.clone(),
            time_bins: report.time_bins.clone(),
            selected_time_bins: report.comparison.time_bins.clone(),
            yield_by_minimum_length: report.comparison.yield_by_minimum_length.clone(),
            distributions: report.distributions.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Tool {
    name: &'static str,
    pub version: &'static str,
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

pub fn write(report: &Report, directory: &Path) -> Result<()> {
    fs::create_dir_all(directory)
        .with_context(|| format!("cannot create output directory {}", directory.display()))?;
    // Catch predictable destination failures before replacing either report.
    for name in ["report.json", "report.html"] {
        let path = directory.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => ensure!(
                metadata.is_file(),
                "report destination is not a regular file: {}",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("cannot inspect {}", path.display()));
            }
        }
    }
    // Finish and sync both temporary files before publishing either. Individual
    // renames replace complete files; the pair is intentionally not a transaction.
    let json = StagedReport::prepare(directory, "report.json", |writer| {
        serde_json::to_writer_pretty(&mut *writer, report)
            .context("cannot serialize JSON report")?;
        writer
            .write_all(b"\n")
            .context("cannot write report newline")?;
        Ok(())
    })?;
    let html = StagedReport::prepare(directory, "report.html", |writer| {
        writer
            .write_all(crate::html::render(report).as_bytes())
            .context("cannot write HTML report")
    })?;
    json.publish()?;
    html.publish()?;
    Ok(())
}

struct StagedReport {
    temporary: PathBuf,
    destination: PathBuf,
}

impl StagedReport {
    fn prepare(
        directory: &Path,
        name: &str,
        render: impl FnOnce(&mut BufWriter<File>) -> Result<()>,
    ) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..32 {
            let temporary = directory.join(format!(
                ".{name}.{}.{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let file = match File::options()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("cannot stage {name} in {}", directory.display())
                    });
                }
            };
            let staged = Self {
                temporary,
                destination: directory.join(name),
            };
            let mut writer = BufWriter::new(file);
            render(&mut writer)?;
            writer
                .flush()
                .with_context(|| format!("cannot flush {name}"))?;
            writer
                .get_ref()
                .sync_all()
                .with_context(|| format!("cannot sync {name}"))?;
            return Ok(staged);
        }
        bail!("cannot create a unique temporary file for {name}")
    }

    fn publish(self) -> Result<()> {
        fs::rename(&self.temporary, &self.destination)
            .with_context(|| format!("cannot replace {}", self.destination.display()))
    }
}

impl Drop for StagedReport {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.temporary);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_render_cleans_both_staged_files_and_preserves_previous_reports() {
        let directory =
            std::env::temp_dir().join(format!("poregauge-write-failure-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("report.json"), "previous JSON").unwrap();
        fs::write(directory.join("report.html"), "previous HTML").unwrap();
        let json = StagedReport::prepare(&directory, "report.json", |writer| {
            writer.write_all(b"new JSON")?;
            Ok(())
        })
        .unwrap();
        let failed = StagedReport::prepare(&directory, "report.html", |writer| {
            writer.write_all(b"partial HTML")?;
            // Flush partial bytes to exercise cleanup of an actually written file.
            writer.flush()?;
            bail!("simulated output failure")
        });
        assert!(failed.is_err());
        drop(json);
        assert_eq!(
            fs::read_to_string(directory.join("report.json")).unwrap(),
            "previous JSON"
        );
        assert_eq!(
            fs::read_to_string(directory.join("report.html")).unwrap(),
            "previous HTML"
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_rename_cleans_temporary_file_without_truncating_destination() {
        let directory =
            std::env::temp_dir().join(format!("poregauge-rename-failure-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let staged = StagedReport::prepare(&directory, "report.html", |writer| {
            writer.write_all(b"complete HTML")?;
            Ok(())
        })
        .unwrap();
        // Simulate the target changing after preflight/staging.
        fs::create_dir(directory.join("report.html")).unwrap();
        fs::write(directory.join("report.html/keep"), "previous data").unwrap();
        assert!(staged.publish().is_err());
        assert_eq!(
            fs::read_to_string(directory.join("report.html/keep")).unwrap(),
            "previous data"
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
