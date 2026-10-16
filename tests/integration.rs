use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

use serde_json::{Value, json};

const HEADER: &str = "run_id\tchannel\tmux\tstart_time\tduration\tpasses_filtering\tsequence_length_template\tmean_qscore_template\tend_reason\n";
const ROW: &str = "synthetic-run\t1\t1\t0\t1\tTRUE\t10\t12.5\tunknown\n";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "poregauge-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn run(&self, input: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_poregauge"))
            .arg("analyze")
            .arg(input)
            .arg("--output")
            .arg(self.0.join("output"))
            .output()
            .unwrap()
    }

    fn analyze(&self, data: &str) -> Value {
        let input = self.0.join("input.tsv");
        fs::write(&input, data).unwrap();
        let output = self.run(&input);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&fs::read(self.0.join("output/report.json")).unwrap()).unwrap()
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn assert_invalid(data: &str, expected: &str) {
    let directory = TestDirectory::new();
    let input = directory.0.join("invalid.tsv");
    fs::write(&input, data).unwrap();
    let output = directory.run(&input);
    assert!(!output.status.success(), "unexpected success for {data:?}");
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains(expected),
        "expected {expected:?}, got {error:?}"
    );
    assert!(!directory.0.join("output/report.json").exists());
}

#[test]
fn complete_synthetic_metrics() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("synthetic.tsv")).unwrap();
    let report = directory.analyze(&data);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(
        report["tool"],
        json!({"name":"PoreGauge", "version":env!("CARGO_PKG_VERSION")})
    );
    assert_eq!(report["run"]["run_id"], "synthetic-run");
    assert_eq!(
        report["metrics"],
        json!({
            "total_records":6, "basecalled_reads":4, "zero_length_records":2,
            "total_yield_bp":100, "passed_reads":2, "failed_reads":2,
            "passed_yield_bp":40, "failed_yield_bp":60,
            "mean_read_length_bp":25.0, "median_read_length_bp":25.0, "n50_bp":30,
            "min_read_length_bp":10, "max_read_length_bp":40
        })
    );
    assert_eq!(report["quality"]["mean_qscore"], 15.0);
    assert_eq!(report["quality"]["median_qscore"], 15.0);
    assert_eq!(report["quality"]["min_qscore"], 0.0);
    assert_eq!(report["quality"]["max_qscore"], 30.0);
    assert_eq!(
        report["quality"]["histogram"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["read_count"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 0, 1, 0, 1, 0, 1, 0, 0]
    );
    let length_bins = report["read_length_distribution"].as_array().unwrap();
    assert_eq!(
        length_bins[0],
        json!({"lower_inclusive_bp":1,"upper_exclusive_bp":1000,"read_count":4,"yield_bp":100})
    );
    assert!(length_bins.iter().skip(1).all(|bin| bin["read_count"] == 0));
    assert_eq!(
        report["channels"],
        json!([
            {"channel":1,"total_records":2,"basecalled_reads":2,"total_yield_bp":60,"passed_reads":0,"passed_yield_bp":0,"mean_qscore":10.0},
            {"channel":2,"total_records":2,"basecalled_reads":2,"total_yield_bp":40,"passed_reads":2,"passed_yield_bp":40,"mean_qscore":20.0},
            {"channel":3,"total_records":2,"basecalled_reads":0,"total_yield_bp":0,"passed_reads":0,"passed_yield_bp":0,"mean_qscore":null}
        ])
    );
    assert_eq!(
        report["muxes"],
        json!([
            {"mux":1,"total_records":2,"basecalled_reads":2,"total_yield_bp":50,"passed_yield_bp":30},
            {"mux":2,"total_records":3,"basecalled_reads":2,"total_yield_bp":50,"passed_yield_bp":10},
            {"mux":3,"total_records":1,"basecalled_reads":0,"total_yield_bp":0,"passed_yield_bp":0}
        ])
    );
    assert_eq!(
        report["time_bins"],
        json!([
            {"start_seconds":0,"end_seconds":900,"total_records":3,"basecalled_reads":2,"yield_bp":60,"cumulative_yield_bp":60},
            {"start_seconds":900,"end_seconds":1800,"total_records":1,"basecalled_reads":1,"yield_bp":30,"cumulative_yield_bp":90},
            {"start_seconds":1800,"end_seconds":2700,"total_records":1,"basecalled_reads":1,"yield_bp":10,"cumulative_yield_bp":100},
            {"start_seconds":2700,"end_seconds":3600,"total_records":1,"basecalled_reads":0,"yield_bp":0,"cumulative_yield_bp":100}
        ])
    );
    assert_eq!(
        report["end_reasons"],
        json!([
            {"end_reason":"future_reason","total_records":2,"basecalled_reads":2,"total_yield_bp":70},
            {"end_reason":"normal","total_records":2,"basecalled_reads":2,"total_yield_bp":30},
            {"end_reason":"signal_positive","total_records":2,"basecalled_reads":0,"total_yield_bp":0}
        ])
    );
}

#[test]
fn reordered_header_and_extra_column() {
    let directory = TestDirectory::new();
    let report = directory.analyze(&fs::read_to_string(fixture("reordered.tsv")).unwrap());
    assert_eq!(report["metrics"]["total_records"], 1);
    assert_eq!(report["metrics"]["passed_reads"], 1);
    assert_eq!(report["metrics"]["n50_bp"], 9);
    assert_eq!(report["metrics"]["median_read_length_bp"], 9.0);
    assert_eq!(report["quality"]["median_qscore"], 12.125);
    assert_eq!(report["channels"][0]["channel"], 6000);
    assert_eq!(report["muxes"][0]["mux"], 9);
    assert_eq!(report["end_reasons"][0]["end_reason"], "unknown_new_reason");
}

#[test]
fn header_only_produces_empty_report() {
    let report = TestDirectory::new().analyze(HEADER);
    assert_eq!(report["metrics"]["total_records"], 0);
    assert_eq!(report["run"]["run_id"], Value::Null);
    for name in [
        "n50_bp",
        "mean_read_length_bp",
        "median_read_length_bp",
        "min_read_length_bp",
        "max_read_length_bp",
    ] {
        assert!(report["metrics"][name].is_null());
    }
    assert!(report["quality"]["mean_qscore"].is_null());
    assert!(report["quality"]["median_qscore"].is_null());
    for name in ["channels", "muxes", "time_bins", "end_reasons"] {
        assert!(report[name].as_array().unwrap().is_empty());
    }
}

#[test]
fn zero_length_records_are_not_q0_reads() {
    let report = TestDirectory::new().analyze(&format!(
        "{HEADER}{}",
        ROW.replace("\t10\t12.5\t", "\t0\t0\t")
    ));
    assert_eq!(report["metrics"]["total_records"], 1);
    assert_eq!(report["metrics"]["zero_length_records"], 1);
    assert_eq!(report["metrics"]["passed_reads"], 0);
    assert_eq!(report["metrics"]["failed_reads"], 0);
    assert_eq!(report["metrics"]["total_yield_bp"], 0);
    assert!(report["quality"]["mean_qscore"].is_null());
    assert!(report["quality"]["median_qscore"].is_null());
    assert!(report["metrics"]["n50_bp"].is_null());
}

#[test]
fn filtering_flag_is_independent_of_qscore_and_end_reason() {
    let low_pass = ROW.replace("12.5", "0");
    let high_fail = ROW.replace("12.5", "50").replace("TRUE", "FALSE");
    let report = TestDirectory::new().analyze(&format!("{HEADER}{low_pass}{high_fail}"));
    assert_eq!(report["metrics"]["passed_reads"], 1);
    assert_eq!(report["metrics"]["failed_reads"], 1);
    assert_eq!(report["metrics"]["passed_yield_bp"], 10);
    assert_eq!(report["metrics"]["failed_yield_bp"], 10);
    assert_eq!(report["quality"]["mean_qscore"], 25.0);
}

#[test]
fn histogram_edges_and_time_gaps() {
    let report = TestDirectory::new().analyze(&format!("{HEADER}synthetic-run\t1\t1\t0\t1\tFALSE\t1000\t5\tx\nsynthetic-run\t1\t1\t3600\t1\tTRUE\t1000000\t50\tx\n"));
    assert_eq!(report["read_length_distribution"][1]["read_count"], 1);
    assert_eq!(report["read_length_distribution"][8]["yield_bp"], 1_000_000);
    assert!(report["read_length_distribution"][8]["upper_exclusive_bp"].is_null());
    assert_eq!(report["quality"]["histogram"][1]["read_count"], 1);
    assert_eq!(report["quality"]["histogram"][8]["read_count"], 1);
    assert_eq!(report["time_bins"].as_array().unwrap().len(), 2);
    assert_eq!(report["time_bins"][1]["cumulative_yield_bp"], 1_001_000);
}

#[test]
fn empty_input_and_missing_columns_fail() {
    assert_invalid("", "empty input");
    let names: Vec<_> = HEADER.trim_end().split('\t').collect();
    for (index, name) in names.iter().enumerate() {
        let header = names
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .map(|(_, value)| *value)
            .collect::<Vec<_>>()
            .join("\t")
            + "\n";
        assert_invalid(&header, &format!("missing required column {name:?}"));
    }
    assert_invalid(&HEADER.replace("channel", "run_id"), "duplicate column");
    assert_invalid(&HEADER.replace("duration", ""), "empty column name");
}

#[test]
fn malformed_numeric_and_boolean_fields_fail_with_line_and_column() {
    for (column, value) in [
        (1, "nope"),
        (1, "0"),
        (1, "-1"),
        (1, "4294967296"),
        (2, "0"),
        (2, "NaN"),
        (3, "-1"),
        (3, "NaN"),
        (3, "inf"),
        (3, "1e100"),
        (4, "-1"),
        (4, "inf"),
        (5, "1"),
        (5, "yes"),
        (6, "-1"),
        (6, "18446744073709551616"),
        (7, "NaN"),
        (7, "inf"),
        (7, "-0.5"),
        (0, ""),
        (8, ""),
    ] {
        let mut row: Vec<_> = ROW.trim_end_matches('\n').split('\t').collect();
        row[column] = value;
        let input = format!("{HEADER}{}\n", row.join("\t"));
        assert_invalid(&input, "line 2");
        if column != 3 || value != "1e100" {
            assert_invalid(&input, HEADER.trim_end().split('\t').nth(column).unwrap());
        }
    }
}

#[test]
fn wrong_field_counts_and_blank_lines_fail() {
    assert_invalid(&format!("{HEADER}synthetic-run\t1\n"), "line: 2");
    assert_invalid(&format!("{HEADER}{}\textra\n", ROW.trim_end()), "line: 2");
    assert_invalid(&format!("{HEADER}\n{ROW}"), "line 2");
    assert_invalid(&format!("{HEADER}{ROW}\n"), "line 3");
    assert_invalid(&format!("\n{HEADER}{ROW}"), "line 1");
    assert_invalid(&format!("\n{}", HEADER.trim_end()), "line 1");
    assert_invalid(&format!("{HEADER}\r\n{ROW}"), "line 2");
}

#[test]
fn invalid_utf8_in_required_field_fails() {
    let directory = TestDirectory::new();
    let mut bytes = format!("{HEADER}{ROW}").into_bytes();
    bytes[HEADER.len()] = 0xff;
    let input = directory.0.join("invalid-utf8.tsv");
    fs::write(&input, bytes).unwrap();
    let output = directory.run(&input);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("line 2"));
    assert!(error.contains("run_id"));
    assert!(error.contains("UTF-8"));
}

#[test]
fn zero_length_fields_still_require_validation_and_consistent_run_id() {
    assert_invalid(
        &format!("{HEADER}{}", ROW.replace("\t10\t12.5\t", "\t0\tNaN\t")),
        "mean_qscore_template",
    );
    let zero = ROW.replace("\t10\t", "\t0\t");
    assert_invalid(
        &format!(
            "{HEADER}{zero}{}",
            zero.replace("synthetic-run", "other-run")
        ),
        "line 3: inconsistent run_id",
    );
}

#[test]
fn extreme_finite_quality_values_remain_finite_in_json() {
    let row = ROW.replace("12.5", &f64::MAX.to_string());
    let report = TestDirectory::new().analyze(&format!("{HEADER}{row}{row}"));
    assert_eq!(report["quality"]["mean_qscore"].as_f64(), Some(f64::MAX));
    assert_eq!(report["quality"]["median_qscore"].as_f64(), Some(f64::MAX));
}

#[test]
fn multiple_runs_and_yield_overflow_fail() {
    assert_invalid(
        &format!("{HEADER}{ROW}{}", ROW.replace("synthetic-run", "other-run")),
        "line 3: inconsistent run_id",
    );
    let max_row = ROW.replace("\t10\t", &format!("\t{}\t", u64::MAX));
    assert_invalid(
        &format!("{HEADER}{max_row}{ROW}"),
        "line 3: total yield exceeds u64",
    );
}

#[test]
fn json_is_byte_for_byte_deterministic() {
    let directory = TestDirectory::new();
    let input = fixture("synthetic.tsv");
    assert!(directory.run(&input).status.success());
    let first = fs::read(directory.0.join("output/report.json")).unwrap();
    assert!(directory.run(&input).status.success());
    let second = fs::read(directory.0.join("output/report.json")).unwrap();
    assert_eq!(first, second);
}

#[test]
fn crlf_and_missing_final_newline_are_supported() {
    let data = format!("{HEADER}{}", ROW.trim_end()).replace('\n', "\r\n");
    assert_eq!(
        TestDirectory::new().analyze(&data)["metrics"]["total_records"],
        1
    );
}

#[test]
fn failed_analysis_preserves_existing_report() {
    let directory = TestDirectory::new();
    let input = directory.0.join("bad.tsv");
    fs::write(&input, format!("{HEADER}invalid\n")).unwrap();
    fs::create_dir(directory.0.join("output")).unwrap();
    let path = directory.0.join("output/report.json");
    fs::write(&path, b"previous report").unwrap();
    assert!(!directory.run(&input).status.success());
    assert_eq!(fs::read(path).unwrap(), b"previous report");
}

#[test]
fn missing_input_and_invalid_output_are_actionable() {
    let directory = TestDirectory::new();
    let missing = directory.run(&directory.0.join("does-not-exist.tsv"));
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("cannot open input"));
    fs::write(directory.0.join("output"), "blocking file").unwrap();
    let output = directory.run(&fixture("synthetic.tsv"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot create output directory"));
}
