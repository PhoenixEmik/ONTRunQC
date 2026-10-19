use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

use serde_json::{Value, json};

#[path = "fixtures/large_tables.rs"]
mod large_tables;

const HEADER: &str = "run_id\tchannel\tmux\tstart_time\tduration\tpasses_filtering\tsequence_length_template\tmean_qscore_template\tend_reason\n";
const ROW: &str = "synthetic-run\t1\t1\t0\t1\tTRUE\t10\t12.5\tunknown\n";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ontrunqc-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn run(&self, input: &Path) -> Output {
        self.run_with_args(input, &[])
    }

    fn run_with_args(&self, input: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
            .arg("analyze")
            .arg(input)
            .args(args)
            .arg("--output")
            .arg(self.0.join("output"))
            .output()
            .unwrap()
    }

    fn analyze(&self, data: &str) -> Value {
        self.analyze_with_args(data, &[])
    }

    fn analyze_with_args(&self, data: &str, args: &[&str]) -> Value {
        let input = self.0.join("input.tsv");
        fs::write(&input, data).unwrap();
        let output = self.run_with_args(&input, args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&fs::read(self.0.join("output/report.json")).unwrap()).unwrap()
    }

    fn html(&self) -> String {
        fs::read_to_string(self.0.join("output/report.html")).unwrap()
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
    assert_invalid_with_args(data, expected, &[]);
}

fn assert_invalid_with_args(data: &str, expected: &str, args: &[&str]) {
    let directory = TestDirectory::new();
    let input = directory.0.join("invalid.tsv");
    fs::write(&input, data).unwrap();
    let output = directory.run_with_args(&input, args);
    assert!(!output.status.success(), "unexpected success for {data:?}");
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains(expected),
        "expected {expected:?}, got {error:?}"
    );
    assert!(!directory.0.join("output/report.json").exists());
    assert!(!directory.0.join("output/report.html").exists());
}

#[test]
fn complete_synthetic_metrics() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("synthetic.tsv")).unwrap();
    let report = directory.analyze(&data);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(
        report["tool"],
        json!({"name":"ONTRunQC", "version":env!("CARGO_PKG_VERSION")})
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
            {"channel":1,"total_records":2,"basecalled_reads":2,"total_yield_bp":60,"passed_reads":0,"passed_yield_bp":0,"mean_qscore":10.0,"mean_read_length_bp":30.0,"mean_passed_read_length_bp":null},
            {"channel":2,"total_records":2,"basecalled_reads":2,"total_yield_bp":40,"passed_reads":2,"passed_yield_bp":40,"mean_qscore":20.0,"mean_read_length_bp":20.0,"mean_passed_read_length_bp":20.0},
            {"channel":3,"total_records":2,"basecalled_reads":0,"total_yield_bp":0,"passed_reads":0,"passed_yield_bp":0,"mean_qscore":null,"mean_read_length_bp":null,"mean_passed_read_length_bp":null}
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
            {"start_seconds":0,"end_seconds":900,"total_records":3,"basecalled_reads":2,"yield_bp":60,"cumulative_yield_bp":60,"passed_reads":0,"passed_yield_bp":0,"mean_read_length_bp":30.0,"mean_passed_read_length_bp":null,"qscore_observations":2,"mean_qscore":10.0,"mean_passed_qscore":null},
            {"start_seconds":900,"end_seconds":1800,"total_records":1,"basecalled_reads":1,"yield_bp":30,"cumulative_yield_bp":90,"passed_reads":1,"passed_yield_bp":30,"mean_read_length_bp":30.0,"mean_passed_read_length_bp":30.0,"qscore_observations":1,"mean_qscore":30.0,"mean_passed_qscore":30.0},
            {"start_seconds":1800,"end_seconds":2700,"total_records":1,"basecalled_reads":1,"yield_bp":10,"cumulative_yield_bp":100,"passed_reads":1,"passed_yield_bp":10,"mean_read_length_bp":10.0,"mean_passed_read_length_bp":10.0,"qscore_observations":1,"mean_qscore":10.0,"mean_passed_qscore":10.0},
            {"start_seconds":2700,"end_seconds":3600,"total_records":1,"basecalled_reads":0,"yield_bp":0,"cumulative_yield_bp":100,"passed_reads":0,"passed_yield_bp":0,"mean_read_length_bp":null,"mean_passed_read_length_bp":null,"qscore_observations":0,"mean_qscore":null,"mean_passed_qscore":null}
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
        if *name == "passes_filtering" {
            continue;
        }
        let header = names
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .map(|(_, value)| *value)
            .collect::<Vec<_>>()
            .join("\t")
            + "\n";
        assert_invalid_with_args(
            &header,
            &format!("missing required column {name:?}"),
            &["--input-schema", "modern"],
        );
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
    let html_path = directory.0.join("output/report.html");
    fs::write(&html_path, b"previous HTML").unwrap();
    assert!(!directory.run(&input).status.success());
    assert_eq!(fs::read(path).unwrap(), b"previous report");
    assert_eq!(fs::read(html_path).unwrap(), b"previous HTML");
}

#[test]
fn html_contains_all_sections_and_matches_json_statistics() {
    let directory = TestDirectory::new();
    let report = directory.analyze(&fs::read_to_string(fixture("synthetic.tsv")).unwrap());
    let html = directory.html();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>ONTRunQC · Sequencing QC report</title>"));
    assert!(html.contains("<h1>ONTRunQC</h1>"));
    assert!(html.contains(&format!("<footer>ONTRunQC {}", env!("CARGO_PKG_VERSION"))));
    for id in [
        "overview",
        "yield",
        "length",
        "quality",
        "channels",
        "mux",
        "termination",
    ] {
        assert!(html.contains(&format!("<section id=\"{id}\"")));
        assert!(html.contains(&format!("href=\"#{id}\"")));
    }
    for key in [
        "total_records",
        "basecalled_reads",
        "total_yield_bp",
        "passed_yield_bp",
        "n50_bp",
        "mean_read_length_bp",
        "median_read_length_bp",
    ] {
        let value = &report["metrics"][key];
        // HTML data attributes preserve the unscaled values, independently of units.
        let expected = value
            .as_u64()
            .map(|v| v.to_string())
            .unwrap_or_else(|| value.as_f64().unwrap().to_string());
        assert!(html.contains(&format!("data-metric=\"{key}\" data-value=\"{expected}\"")));
    }
    for key in ["mean_qscore", "median_qscore"] {
        let expected = report["quality"][key].as_f64().unwrap().to_string();
        assert!(html.contains(&format!("data-metric=\"{key}\" data-value=\"{expected}\"")));
    }
    assert!(html.contains("data-metric=\"pass_rate_percent\" data-value=\"50\""));
    assert!(html.contains("0.0000001 Gb"));
    assert!(html.contains("0.03 kb"));
    assert!(html.contains("0.025 kb"));
    assert!(html.contains("[1, 1,000)"));
    assert!(html.contains("future_reason"));
    assert!(html.contains("data-channel=\"3\" data-yield=\"0\" data-reads=\"0\" data-qscore=\"\""));
    assert!(!html.contains("read-1"));
    assert!(!html.contains("synthetic.fastq"));
}

#[test]
fn html_has_no_external_resource_dependencies() {
    let directory = TestDirectory::new();
    directory.analyze(&format!("{HEADER}{ROW}"));
    let html = directory.html();
    for forbidden in [
        "http://",
        "https://",
        "src=",
        "@import",
        "url(",
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "<link",
    ] {
        assert!(!html.contains(forbidden), "found dependency: {forbidden}");
    }
    for href in html.split("href=\"").skip(1) {
        assert!(href.starts_with('#'));
    }
    assert!(html.contains("<style>"));
    assert!(html.contains("<svg"));
    assert_eq!(html.matches("<script>").count(), 1);
    assert!(html.contains("<noscript>"));
}

#[test]
fn table_alignment_distinguishes_labels_from_numeric_identifiers() {
    let directory = TestDirectory::new();
    directory.analyze(&fs::read_to_string(fixture("synthetic.tsv")).unwrap());
    let html = directory.html();
    for label in [
        "Start–end (s)",
        "Length interval (bp)",
        "Q-score interval",
        "End reason",
    ] {
        assert!(html.contains(&format!("<th class=\"label\" scope=\"col\">{label}</th>")));
    }
    for numeric_header in [
        "Channel",
        "Mux",
        "Records",
        "Basecalled reads",
        "Yield (bp)",
    ] {
        assert!(html.contains(&format!("<th scope=\"col\">{numeric_header}</th>")));
        assert!(!html.contains(&format!(
            "<th class=\"label\" scope=\"col\">{numeric_header}</th>"
        )));
    }
    let channels = html
        .split("id=\"channel-table\"")
        .nth(1)
        .unwrap()
        .split("</table>")
        .next()
        .unwrap();
    assert!(!channels.contains("class=\"label\""));
    assert!(channels.contains("<th scope=\"row\">1</th>"));
    assert!(channels.contains("<td>Not available</td>"));
}

#[test]
fn termination_bars_and_accessible_counts_share_the_correct_table_cell() {
    let directory = TestDirectory::new();
    let reason = "unknown_reason_with_a_very_long_unbroken_name_".repeat(8);
    directory.analyze(&format!("{HEADER}{}", ROW.replace("unknown", &reason)));
    let html = directory.html();
    let table = html
        .split("<table class=\"reason-table\">")
        .nth(1)
        .unwrap()
        .split("</table>")
        .next()
        .unwrap();
    assert!(table.contains(&format!(
        "<th class=\"label\" scope=\"row\"><span class=\"reason-name\">{reason}</span></th>"
    )));
    let cell = table
        .split("<td class=\"bar-cell\">")
        .nth(1)
        .unwrap()
        .split("</td>")
        .next()
        .unwrap();
    assert!(cell.starts_with("<span class=\"record-count\">1</span>"));
    assert!(
        cell.contains("class=\"table-bar\" style=\"--bar-width:100.00%\" aria-hidden=\"true\"")
    );
    assert!(cell.ends_with("</span>"));
    assert!(table.contains("</span></td><td>1</td><td>10</td>"));
    assert!(!table.contains("<svg"));
}

#[test]
fn metadata_cannot_inject_html_or_scripts() {
    let directory = TestDirectory::new();
    let run = "run\"><img src=x onerror=alert(1)>&'";
    let reason = "</script><script>alert('x')</script><svg onload=alert(1)>";
    let data = format!(
        "{HEADER}{}",
        ROW.replace("synthetic-run", run).replace("unknown", reason)
    );
    let report = directory.analyze(&data);
    let html = directory.html();
    assert_eq!(report["run"]["run_id"], run);
    assert_eq!(report["end_reasons"][0]["end_reason"], reason);
    assert!(html.contains("run&quot;&gt;&lt;img src=x onerror=alert(1)&gt;&amp;&#39;"));
    assert!(html.contains(
        "&lt;/script&gt;&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;&lt;svg onload=alert(1)&gt;"
    ));
    assert!(!html.contains("<img"));
    assert!(!html.contains("<svg onload"));
    assert_eq!(html.matches("<script>").count(), 1);
    assert_eq!(html.matches("</script>").count(), 1);
}

#[test]
fn empty_and_zero_length_only_html_show_unavailable_statistics() {
    for data in [
        HEADER.to_owned(),
        format!("{HEADER}{}", ROW.replace("\t10\t12.5\t", "\t0\t0\t")),
    ] {
        let directory = TestDirectory::new();
        directory.analyze(&data);
        let html = directory.html();
        for key in [
            "n50_bp",
            "mean_read_length_bp",
            "mean_qscore",
            "median_qscore",
            "median_read_length_bp",
            "pass_rate_percent",
        ] {
            assert!(
                html.contains(&format!("data-metric=\"{key}\" data-value=\"\"")),
                "missing unavailable {key}"
            );
        }
        assert!(html.contains("No basecalled reads are available."));
        assert!(html.contains("No basecalled channel yield available."));
        assert!(html.contains("Not available"));
        assert!(!html.contains("NaN"));
        assert!(!html.contains("Infinity"));
        assert!(!html.contains("height=\"-"));
    }
}

#[test]
fn html_time_bins_preserve_gaps_and_relative_units() {
    let directory = TestDirectory::new();
    let second = ROW.replace("\t0\t1\tTRUE", "\t3600\t1\tTRUE");
    let report = directory.analyze(&format!("{HEADER}{ROW}{second}"));
    assert_eq!(report["time_bins"].as_array().unwrap().len(), 2);
    let html = directory.html();
    assert!(html.contains("Time from run start (h)"));
    assert!(html.contains("[0, 900) seconds: 10 bp"));
    assert!(html.contains("Through 4500 seconds: 20 bp cumulative"));
    assert!(html.contains("[3600, 4500) seconds: 10 bp"));
    assert!(!html.contains("[900, 1800) seconds:"));
    assert!(html.contains("Cumulative yield steps at interval ends"));
}

#[test]
fn html_handles_thousands_of_channels_without_a_channel_bar_chart() {
    let directory = TestDirectory::new();
    let mut data = HEADER.to_owned();
    for channel in 1..=4096 {
        data.push_str(&format!(
            "synthetic-run\t{channel}\t1\t0\t1\tTRUE\t10\t12\tunknown\n"
        ));
    }
    directory.analyze(&data);
    let html = directory.html();
    for id in ["channel-table", "channel-mux-table"] {
        let table = html
            .split(&format!("id=\"{id}\""))
            .nth(1)
            .unwrap()
            .split("</table>")
            .next()
            .unwrap();
        assert_eq!(table.matches("<tr data-channel=").count(), 4096);
    }
    assert_eq!(html.matches("class=\"channel-point\"").count(), 4096);
    assert!(html.contains("data-channel=\"4096\""));
    assert!(html.contains("id=\"channel-pagination\""));
    assert!(html.contains("id=\"channel-search\""));
    assert!(
        html.len() < 8_000_000,
        "unexpectedly large aggregated report"
    );
}

#[test]
fn html_is_deterministic_and_replacements_leave_no_temporary_files() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("synthetic.tsv")).unwrap();
    directory.analyze(&data);
    let first = directory.html();
    directory.analyze(&data);
    assert_eq!(first, directory.html());
    let files: Vec<_> = fs::read_dir(directory.0.join("output"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(files.len(), 2);
    assert!(
        files
            .iter()
            .all(|name| name == "report.json" || name == "report.html")
    );
}

#[test]
fn html_preserves_large_integer_yields_without_nonfinite_plot_coordinates() {
    let directory = TestDirectory::new();
    let first = ROW.replace("\t10\t", "\t9007199254740993\t");
    let second = ROW
        .replace("\t10\t", "\t9007199254740992\t")
        .replace("\t1\t1\t", "\t2\t1\t");
    let report = directory.analyze(&format!("{HEADER}{first}{second}"));
    assert_eq!(
        report["metrics"]["total_yield_bp"],
        18_014_398_509_481_985_u64
    );
    let html = directory.html();
    assert!(html.contains("data-yield=\"9007199254740993\""));
    assert!(html.contains("data-yield=\"9007199254740992\""));
    assert!(html.contains("9,007,199,254,740,993"));
    assert!(!html.contains("NaN"));
    assert!(!html.contains("Infinity"));
}

#[test]
fn blocked_html_destination_preserves_existing_json_and_cleans_staging() {
    let directory = TestDirectory::new();
    directory.analyze(&format!("{HEADER}{ROW}"));
    let json_path = directory.0.join("output/report.json");
    let previous = fs::read(&json_path).unwrap();
    let html_path = directory.0.join("output/report.html");
    fs::remove_file(&html_path).unwrap();
    fs::create_dir(&html_path).unwrap();
    let input = directory.0.join("updated.tsv");
    fs::write(&input, format!("{HEADER}{ROW}{ROW}")).unwrap();
    let output = directory.run(&input);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a regular file"));
    assert_eq!(previous, fs::read(json_path).unwrap());
    assert_eq!(fs::read_dir(directory.0.join("output")).unwrap().count(), 2);
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

#[test]
fn deep_diagnostics_have_exact_aggregates_and_conserve_yield() {
    let directory = TestDirectory::new();
    let report = directory.analyze(&fs::read_to_string(fixture("deep_qc.tsv")).unwrap());
    let d = &report["diagnostics"];
    assert_eq!(report["metrics"]["total_records"], 10);
    assert_eq!(report["metrics"]["basecalled_reads"], 7);
    assert_eq!(report["metrics"]["zero_length_records"], 3);
    assert_eq!(report["metrics"]["total_yield_bp"], 2150);
    assert_eq!(report["metrics"]["passed_yield_bp"], 1450);
    assert_eq!(d["physical_layout"], Value::Null);
    assert_eq!(
        d["channel_activity"],
        json!([
            {"start_seconds":0,"basecalled_producing_channels":2},
            {"start_seconds":900,"basecalled_producing_channels":1},
            {"start_seconds":1800,"basecalled_producing_channels":0},
            {"start_seconds":2700,"basecalled_producing_channels":2},
            {"start_seconds":3600,"basecalled_producing_channels":0}
        ])
    );
    let groups = d["channel_mux"].as_array().unwrap();
    assert_eq!(groups.len(), 8);
    assert_eq!(
        groups[0],
        json!({"channel":1,"mux":1,"total_records":2,"basecalled_reads":2,"zero_length_records":0,"total_yield_bp":150,"passed_reads":2,"passed_yield_bp":150})
    );
    let unusual = groups
        .iter()
        .find(|m| m["channel"] == 10 && m["mux"] == 9)
        .unwrap();
    assert_eq!(unusual["total_records"], 2);
    assert_eq!(unusual["basecalled_reads"], 1);
    assert_eq!(unusual["zero_length_records"], 1);
    assert_eq!(unusual["total_yield_bp"], 300);
    for (array, yield_field) in [
        (&report["channels"], "total_yield_bp"),
        (&d["channel_mux"], "total_yield_bp"),
        (&report["time_bins"], "yield_bp"),
    ] {
        assert_eq!(
            array
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m[yield_field].as_u64().unwrap())
                .sum::<u64>(),
            2150
        );
    }
    assert_eq!(
        groups
            .iter()
            .map(|m| m["passed_yield_bp"].as_u64().unwrap())
            .sum::<u64>(),
        1450
    );
    assert_eq!(
        groups
            .iter()
            .map(|m| m["total_records"].as_u64().unwrap())
            .sum::<u64>(),
        10
    );
    assert!(
        groups
            .iter()
            .all(|m| m["basecalled_reads"].as_u64().unwrap()
                + m["zero_length_records"].as_u64().unwrap()
                == m["total_records"].as_u64().unwrap())
    );
    let speed = &d["sequencing_speed"];
    assert_eq!(speed["eligible_reads"], 5);
    assert_eq!(speed["mean_bp_per_second"], 300.0);
    assert_eq!(
        speed["histogram"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["read_count"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![0, 1, 0, 2, 2, 0, 0, 0, 0]
    );
    assert_eq!(
        speed["time_bins"],
        json!([
            {"start_seconds":0,"eligible_reads":4,"mean_bp_per_second":300.0},
            {"start_seconds":900,"eligible_reads":0,"mean_bp_per_second":null},
            {"start_seconds":1800,"eligible_reads":0,"mean_bp_per_second":null},
            {"start_seconds":2700,"eligible_reads":1,"mean_bp_per_second":300.0},
            {"start_seconds":3600,"eligible_reads":0,"mean_bp_per_second":null}
        ])
    );
    let html = directory.html();
    assert!(html.contains("data-metric=\"speed_eligible_reads\" data-value=\"5\""));
    assert!(html.contains("data-metric=\"mean_speed_bp_per_second\" data-value=\"300\""));
    assert!(html.contains("Channels producing basecalled reads over time"));
    assert!(html.contains("id=\"channel-mux-table\""));
    assert!(html.contains("Passed yield by mux"));
    assert!(html.contains("[100, 200) bp/s"));
    // Speed has missing observations in intermediate bins: no connecting line.
    let speed_plot = html
        .split("id=\"speed-time-caption\"")
        .nth(1)
        .unwrap()
        .split("</figure>")
        .next()
        .unwrap();
    let path = speed_plot
        .split("class=\"cumulative-line\" d=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(path.matches('M').count(), 2);
    assert!(!path.contains('L'));
}

#[test]
fn explicit_layout_heatmaps_conserve_yields_and_preserve_unobserved_states() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("deep_qc.tsv")).unwrap();
    let args = ["--flowcell-layout", "promethion-3000"];
    let report = directory.analyze_with_args(&data, &args);
    let layout = &report["diagnostics"]["physical_layout"];
    assert_eq!(layout["name"], "promethion-3000");
    assert_eq!(layout["rows"], 25);
    assert_eq!(layout["columns"], 120);
    assert_eq!(
        layout["channel_coordinates"].as_array().unwrap().len(),
        3000
    );
    assert_eq!(
        layout["channel_coordinates"][250],
        json!({"channel":251,"row":0,"column":10})
    );
    let first_html = directory.html();
    let mut ids = std::collections::BTreeSet::new();
    for part in first_html.split(" id=\"").skip(1) {
        let id = part.split('"').next().unwrap();
        assert!(ids.insert(id), "duplicate HTML/SVG ID: {id}");
    }
    for part in first_html.split("aria-labelledby=\"").skip(1) {
        for id in part.split('"').next().unwrap().split_whitespace() {
            assert!(ids.contains(id), "missing accessible label: {id}");
        }
    }
    for (id, total) in [
        ("spatial-total-yield", 2150),
        ("spatial-passed-yield", 1450),
    ] {
        let map = first_html
            .split(&format!("id=\"{id}-caption\""))
            .nth(1)
            .unwrap()
            .split("</figure>")
            .next()
            .unwrap();
        assert_eq!(map.matches("class=\"heatmap-cell\"").count(), 3000);
        let sum: u64 = map
            .split("data-yield-bp=\"")
            .skip(1)
            .filter_map(|part| part.split('"').next().unwrap().parse::<u64>().ok())
            .sum();
        assert_eq!(sum, total);
        assert!(map.contains("data-channel=\"250\" data-yield-bp=\"0\""));
        assert!(map.contains("data-channel=\"2\" data-yield-bp=\"\""));
        assert!(map.contains("Observed · zero yield"));
        assert!(map.contains("Not observed · unavailable yield"));
    }
    for forbidden in [
        "http://",
        "https://",
        "src=",
        "@import",
        "fetch(",
        "XMLHttpRequest",
        "<link",
    ] {
        assert!(
            !first_html.contains(forbidden),
            "external resource: {forbidden}"
        );
    }
    let first_json = fs::read(directory.0.join("output/report.json")).unwrap();
    directory.analyze_with_args(&data, &args);
    assert_eq!(first_html, directory.html());
    assert_eq!(
        first_json,
        fs::read(directory.0.join("output/report.json")).unwrap()
    );
    assert!(first_html.len() < 4_000_000);
}

#[test]
fn layout_is_opt_in_and_rejects_every_incompatible_record() {
    let directory = TestDirectory::new();
    let data = format!("{HEADER}{}", ROW.replace("\t1\t1\t0\t", "\t3001\t1\t0\t"));
    let input = directory.0.join("invalid-layout.tsv");
    for zero_length in [false, true] {
        let data = if zero_length {
            data.replace("\t10\t", "\t0\t")
        } else {
            data.clone()
        };
        fs::write(&input, &data).unwrap();
        let output = directory.run_with_args(&input, &["--flowcell-layout", "promethion-3000"]);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("line 2"));
        assert!(error.contains("channel 3001"));
        assert!(error.contains("expected 1–3000"));
        assert!(!directory.0.join("output").exists());
        let report = directory.analyze(&data);
        assert_eq!(report["diagnostics"]["physical_layout"], Value::Null);
        assert!(!directory.html().contains("class=\"heatmap-cell\""));
        fs::remove_dir_all(directory.0.join("output")).unwrap();
    }
    let unknown = directory.run_with_args(&input, &["--flowcell-layout", "unknown"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("promethion-3000"));
}

#[test]
fn optional_duration_is_safe_and_does_not_fall_back_to_duration() {
    let directory = TestDirectory::new();
    let absent = directory.analyze(&format!("{HEADER}{ROW}"));
    assert_eq!(absent["diagnostics"]["sequencing_speed"], Value::Null);
    assert!(
        directory
            .html()
            .contains("the input has no <code>template_duration</code> column")
    );
    let header = format!("template_duration\t{HEADER}");
    for value in ["", "0", "-1", "NaN", "inf", "-inf", "invalid", "1e-320"] {
        let report = directory.analyze(&format!("{header}{value}\t{ROW}"));
        let speed = &report["diagnostics"]["sequencing_speed"];
        assert_eq!(report["metrics"]["basecalled_reads"], 1);
        assert_eq!(speed["eligible_reads"], 0, "{value}");
        assert_eq!(speed["mean_bp_per_second"], Value::Null);
        assert!(
            speed["histogram"]
                .as_array()
                .unwrap()
                .iter()
                .all(|b| b["read_count"] == 0)
        );
        assert!(
            directory
                .html()
                .contains("No eligible sequencing speed observations")
        );
    }
    let valid = directory.analyze(&format!("{header}0.5\t{ROW}"));
    assert_eq!(
        valid["diagnostics"]["sequencing_speed"]["mean_bp_per_second"],
        20.0
    );
    // A valid positive template duration cannot make a zero-length row eligible.
    let zero = directory.analyze(&format!("{header}1\t{}", ROW.replace("\t10\t", "\t0\t")));
    assert_eq!(zero["diagnostics"]["sequencing_speed"]["eligible_reads"], 0);
}

#[test]
fn speed_extremes_and_fixed_histogram_boundaries_remain_finite() {
    let directory = TestDirectory::new();
    let header = format!("template_duration\t{HEADER}");
    let mut data = header.clone();
    for speed in [
        50.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 800.0, 1000.0,
    ] {
        data.push_str(&format!("{}\t{ROW}", 10.0 / speed));
    }
    let report = directory.analyze(&data);
    let histogram = report["diagnostics"]["sequencing_speed"]["histogram"]
        .as_array()
        .unwrap();
    assert_eq!(
        histogram
            .iter()
            .map(|b| b["read_count"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1; 9]
    );
    for duration in ["1e-307", "1e308"] {
        let report = directory.analyze(&format!("{header}{duration}\t{ROW}"));
        let speed = report["diagnostics"]["sequencing_speed"]["mean_bp_per_second"]
            .as_f64()
            .unwrap();
        assert!(speed.is_finite() && speed > 0.0);
        let html = directory.html();
        for invalid in ["NaN", "Infinity", "=\"inf", " inf"] {
            assert!(!html.contains(invalid), "{invalid} at {duration}");
        }
    }
}

#[test]
fn termination_diagnostics_have_explicit_denominators_and_deterministic_order() {
    let directory = TestDirectory::new();
    let report = directory.analyze(&fs::read_to_string(fixture("deep_qc.tsv")).unwrap());
    let reasons = report["diagnostics"]["termination_reasons"]
        .as_array()
        .unwrap();
    assert_eq!(
        reasons
            .iter()
            .map(|r| r["end_reason"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["normal", "zero", "other", "unfamiliar_reason"]
    );
    let normal = &reasons[0];
    assert_eq!(normal["summary_record_percent"], 40.0);
    assert_eq!(normal["zero_length_fraction"], 0.0);
    assert_eq!(normal["basecalled_read_fraction"], 1.0);
    assert_eq!(normal["passed_fraction_of_basecalled"], 0.5);
    assert_eq!(normal["passed_reads"], 2);
    assert!(
        (normal["total_yield_percent"].as_f64().unwrap() - 100.0 * 850.0 / 2150.0).abs() < 1e-12
    );
    assert_eq!(reasons[1]["zero_length_fraction"], 1.0);
    assert_eq!(reasons[1]["passed_fraction_of_basecalled"], Value::Null);
    assert_eq!(reasons[1]["total_yield_percent"], 0.0);
    let html = directory.html();
    let table = html.split("class=\"reason-table\"").nth(1).unwrap();
    assert!(table.find(">normal</span>").unwrap() < table.find(">zero</span>").unwrap());
    assert!(table.contains("Passed / basecalled (%)"));
    let zero = directory.analyze(&format!("{HEADER}{}", ROW.replace("\t10\t", "\t0\t")));
    assert_eq!(
        zero["diagnostics"]["termination_reasons"][0]["total_yield_percent"],
        Value::Null
    );
    // Existing alphabetical order of the original end_reasons array is retained.
    let tied = directory.analyze(&fs::read_to_string(fixture("synthetic.tsv")).unwrap());
    assert_eq!(
        tied["diagnostics"]["termination_reasons"][0]["end_reason"],
        "future_reason"
    );
}

#[test]
fn empty_deep_reports_and_metadata_are_safe() {
    let directory = TestDirectory::new();
    let header = format!("template_duration\t{HEADER}");
    let report = directory.analyze_with_args(&header, &["--flowcell-layout", "promethion-3000"]);
    assert_eq!(report["diagnostics"]["channel_activity"], json!([]));
    assert_eq!(report["diagnostics"]["channel_mux"], json!([]));
    assert_eq!(report["diagnostics"]["termination_reasons"], json!([]));
    assert_eq!(
        report["diagnostics"]["sequencing_speed"]["eligible_reads"],
        0
    );
    assert!(directory.html().contains("No observed channel/mux pairs."));
    assert_eq!(
        directory.html().matches("class=\"heatmap-cell\"").count(),
        9000
    );
    let reason = "</script><img src=x onerror=alert(1)>";
    let data = format!("{header}1\t{}", ROW.replace("unknown", reason));
    let report = directory.analyze_with_args(&data, &["--flowcell-layout", "promethion-3000"]);
    assert_eq!(
        report["diagnostics"]["termination_reasons"][0]["end_reason"],
        reason
    );
    let html = directory.html();
    assert!(!html.contains(reason));
    assert!(!html.contains("<img"));
    assert!(html.contains("&lt;/script&gt;&lt;img"));
    assert_eq!(html.matches("<script>").count(), 1);
}

#[test]
fn activity_counts_distinct_channels_in_unsorted_bins_and_preserves_gaps() {
    let directory = TestDirectory::new();
    let row = |channel, time, length| {
        format!("synthetic-run\t{channel}\t9\t{time}\t1\tTRUE\t{length}\t12\tunknown\n")
    };
    let data = format!(
        "{HEADER}{}{}{}{}{}{}",
        row(1, 3600, 10),
        row(1, 0, 10),
        row(1, 100, 10),
        row(2, 3600, 10),
        row(1, 3700, 10),
        row(3, 900, 0)
    );
    let report = directory.analyze(&data);
    assert_eq!(
        report["diagnostics"]["channel_activity"],
        json!([
            {"start_seconds":0,"basecalled_producing_channels":1},
            {"start_seconds":900,"basecalled_producing_channels":0},
            {"start_seconds":3600,"basecalled_producing_channels":2}
        ])
    );
    let html = directory.html();
    let chart = html
        .split("id=\"channel-activity-caption\"")
        .nth(1)
        .unwrap()
        .split("</figure>")
        .next()
        .unwrap();
    let path = chart
        .split("class=\"cumulative-line\" d=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(path.matches('M').count(), 2);
    assert_eq!(path.matches('L').count(), 1);
}

fn assert_number(value: &Value, expected: f64) {
    let actual = value.as_f64().unwrap();
    assert!(
        (actual - expected).abs() <= 1e-12 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

fn matrix_total(matrix: &Value) -> u64 {
    matrix
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|row| row.as_array().unwrap())
        .map(|v| v.as_u64().unwrap())
        .sum()
}

#[test]
fn advanced_quality_fixture_has_exact_subset_time_and_channel_statistics() {
    let directory = TestDirectory::new();
    let report = directory.analyze(&fs::read_to_string(fixture("advanced_quality.tsv")).unwrap());
    let metrics = &report["metrics"];
    assert_eq!(metrics["total_records"], 11);
    assert_eq!(metrics["basecalled_reads"], 8);
    assert_eq!(metrics["zero_length_records"], 3);
    assert_eq!(metrics["total_yield_bp"], 180);
    assert_eq!(metrics["passed_reads"], 4);
    assert_eq!(metrics["passed_yield_bp"], 100);
    assert_eq!(metrics["n50_bp"], 30);
    assert_eq!(metrics["median_read_length_bp"], 22.5);
    assert_number(&report["quality"]["mean_qscore"], 17.5);
    assert_eq!(report["quality"]["median_qscore"], 17.5);
    let passed = &report["diagnostics"]["passed_read_statistics"];
    assert_eq!(
        passed,
        &json!({
            "read_count":4,"total_yield_bp":100,"mean_read_length_bp":25.0,
            "median_read_length_bp":25.0,"n50_bp":30,"min_read_length_bp":10,
            "max_read_length_bp":40,"mean_qscore":15.0,"median_qscore":15.0
        })
    );
    let bins = report["time_bins"].as_array().unwrap();
    assert_eq!(
        bins.iter()
            .map(|b| b["start_seconds"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [0, 900, 1800, 3600, 4500]
    );
    for (index, count, bases, pass_count, pass_bases, mean, pass_mean, q, pass_q) in [
        (0, 3, 35, 2, 30, 35.0 / 3.0, 15.0, 5.0, 5.0),
        (1, 2, 45, 1, 30, 22.5, 30.0, 17.5, 20.0),
        (3, 3, 100, 1, 40, 100.0 / 3.0, 40.0, 30.0, 30.0),
    ] {
        let bin = &bins[index];
        assert_eq!(bin["basecalled_reads"], count);
        assert_eq!(bin["qscore_observations"], count);
        assert_eq!(bin["yield_bp"], bases);
        assert_eq!(bin["passed_reads"], pass_count);
        assert_eq!(bin["passed_yield_bp"], pass_bases);
        assert_number(&bin["mean_read_length_bp"], mean);
        assert_number(&bin["mean_passed_read_length_bp"], pass_mean);
        assert_number(&bin["mean_qscore"], q);
        assert_number(&bin["mean_passed_qscore"], pass_q);
    }
    for index in [2, 4] {
        for key in [
            "mean_read_length_bp",
            "mean_passed_read_length_bp",
            "mean_qscore",
            "mean_passed_qscore",
        ] {
            assert!(bins[index][key].is_null());
        }
        assert_eq!(bins[index]["qscore_observations"], 0);
    }
    for (bin_key, run_key) in [
        ("basecalled_reads", "basecalled_reads"),
        ("passed_reads", "passed_reads"),
        ("yield_bp", "total_yield_bp"),
        ("passed_yield_bp", "passed_yield_bp"),
    ] {
        assert_eq!(
            bins.iter()
                .map(|b| b[bin_key].as_u64().unwrap())
                .sum::<u64>(),
            metrics[run_key].as_u64().unwrap()
        );
    }
    let channels = report["channels"].as_array().unwrap();
    for (i, mean, pass_mean) in [
        (0, 15.0, Some(20.0)),
        (1, 30.0, Some(30.0)),
        (2, 25.0, None),
    ] {
        assert_number(&channels[i]["mean_read_length_bp"], mean);
        if let Some(value) = pass_mean {
            assert_number(&channels[i]["mean_passed_read_length_bp"], value);
        } else {
            assert!(channels[i]["mean_passed_read_length_bp"].is_null());
        }
    }
    assert!(channels[3]["mean_read_length_bp"].is_null());
    let distribution = report["diagnostics"]["channel_read_length_distribution"]
        .as_array()
        .unwrap();
    assert_eq!(
        distribution
            .iter()
            .map(|b| b["channel_count"].as_u64().unwrap())
            .sum::<u64>(),
        3
    );
    assert_eq!(
        distribution
            .iter()
            .map(|b| b["passed_channel_count"].as_u64().unwrap())
            .sum::<u64>(),
        2
    );
    let joint = &report["diagnostics"]["length_quality_histogram"];
    assert_eq!(matrix_total(&joint["all_read_counts"]), 8);
    assert_eq!(matrix_total(&joint["passed_read_counts"]), 4);
}

#[test]
fn length_quality_bins_include_boundaries_and_unbounded_tails_without_thresholds() {
    let directory = TestDirectory::new();
    // (length, Q-score, expected length column, expected Q row)
    let observations = [
        (1, "0", 0, 0),
        (2, "2", 1, 1),
        (3, "2", 1, 1),
        (4, "4", 2, 2),
        (1023, "49.999", 9, 24),
        (1024, "50", 10, 25),
        (1048575, "100", 19, 25),
        (1048576, "1e308", 20, 25),
        (1048577, "0", 20, 0),
    ];
    let mut data = HEADER.to_owned();
    let mut counts = vec![vec![0_u64; 21]; 26];
    let mut passed = counts.clone();
    for (i, (length, q, l, row)) in observations.into_iter().enumerate() {
        let flag = i % 2 == 0;
        data.push_str(&format!(
            "fictional\t1\t1\t0\t1\t{}\t{length}\t{q}\tnormal\n",
            if flag { "TRUE" } else { "FALSE" }
        ));
        counts[row][l] += 1;
        passed[row][l] += u64::from(flag);
    }
    let report = directory.analyze(&data);
    let h = &report["diagnostics"]["length_quality_histogram"];
    assert_eq!(
        h["length_lower_bounds_bp"],
        json!((0..=20).map(|i| 1_u64 << i).collect::<Vec<_>>())
    );
    assert_eq!(
        h["qscore_lower_bounds"],
        json!((0..=25).map(|i| i as f64 * 2.0).collect::<Vec<_>>())
    );
    assert_eq!(h["all_read_counts"], json!(counts));
    assert_eq!(h["passed_read_counts"], json!(passed));
    assert_eq!(report["metrics"]["passed_reads"], 5);
    assert!(
        report["quality"]["mean_qscore"]
            .as_f64()
            .unwrap()
            .is_finite()
    );
    let html = directory.html();
    assert!(html.contains("open-ended overflow bins"));
    assert!(html.contains("log₁₀(1 + count)"));
    assert!(!html.contains("NaN") && !html.contains("Infinity"));
    let huge = TestDirectory::new().analyze(&format!(
        "{HEADER}{}",
        ROW.replace("\t10\t12.5\t", &format!("\t{}\t{}\t", u64::MAX, f64::MAX))
    ));
    let huge_h = &huge["diagnostics"]["length_quality_histogram"];
    assert_eq!(huge_h["all_read_counts"][25][20], 1);
    assert_eq!(huge_h["passed_read_counts"][25][20], 1);
    assert_eq!(
        huge["diagnostics"]["passed_read_statistics"]["n50_bp"],
        u64::MAX
    );
}

#[test]
fn advanced_empty_zero_length_and_no_passed_subsets_are_unavailable() {
    for rows in [
        String::new(),
        ROW.replace("\t10\t12.5\t", "\t0\t1e308\t"),
        ROW.replace("TRUE", "FALSE"),
    ] {
        let directory = TestDirectory::new();
        let report = directory.analyze(&format!("{HEADER}{rows}"));
        let passed = &report["diagnostics"]["passed_read_statistics"];
        assert_eq!(passed["read_count"], 0);
        assert_eq!(passed["total_yield_bp"], 0);
        for key in [
            "mean_read_length_bp",
            "median_read_length_bp",
            "n50_bp",
            "min_read_length_bp",
            "max_read_length_bp",
            "mean_qscore",
            "median_qscore",
        ] {
            assert!(passed[key].is_null(), "{key}");
        }
        assert_eq!(
            matrix_total(&report["diagnostics"]["length_quality_histogram"]["passed_read_counts"]),
            0
        );
        let html = directory.html();
        assert!(html.contains("Not available"));
        assert!(html.contains("No basecalled observations in this subset."));
        assert!(html.contains("data-metric=\"passed_read_count\" data-value=\"0\""));
    }
    assert_invalid("", "empty");
}

#[test]
fn advanced_time_series_preserve_independent_gaps_and_valid_q_zero() {
    let directory = TestDirectory::new();
    let row = |time, passed, length, q| {
        format!("fictional\t1\t1\t{time}\t1\t{passed}\t{length}\t{q}\tnormal\n")
    };
    let report = directory.analyze(&format!(
        "{HEADER}{}{}{}{}{}",
        row(4500, "TRUE", 40, 30),
        row(900, "FALSE", 20, 10),
        row(2700, "TRUE", 0, 0),
        row(0, "TRUE", 10, 0),
        row(1800, "TRUE", 30, 20)
    ));
    assert_eq!(report["time_bins"][0]["mean_qscore"], 0.0);
    assert_eq!(report["time_bins"][0]["mean_passed_qscore"], 0.0);
    assert!(report["time_bins"][1]["mean_passed_qscore"].is_null());
    let html = directory.html();
    for id in ["length-time", "qscore-time"] {
        let figure = html
            .split(&format!("id=\"{id}-caption\""))
            .nth(1)
            .unwrap()
            .split("</figure>")
            .next()
            .unwrap();
        for (class, moves, lines) in [("cumulative-line", 2, 2), ("passed-line", 3, 0)] {
            let path = figure
                .split(&format!("class=\"{class}\" d=\""))
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            assert_eq!(path.matches('M').count(), moves, "{id}: {path}");
            assert_eq!(path.matches('L').count(), lines, "{id}: {path}");
        }
    }
}

#[test]
fn advanced_reports_are_deterministic_offline_and_match_json_values() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("advanced_quality.tsv")).unwrap();
    let report = directory.analyze(&data);
    let html = directory.html();
    let json_bytes = fs::read(directory.0.join("output/report.json")).unwrap();
    for key in [
        "read_count",
        "total_yield_bp",
        "n50_bp",
        "mean_read_length_bp",
        "median_read_length_bp",
        "mean_qscore",
        "median_qscore",
    ] {
        let value = &report["diagnostics"]["passed_read_statistics"][key];
        let value = value
            .as_u64()
            .map(|v| v.to_string())
            .unwrap_or_else(|| value.as_f64().unwrap().to_string());
        assert!(html.contains(&format!(
            "data-metric=\"passed_{key}\" data-value=\"{value}\""
        )));
    }
    for (id, expected) in [("length-quality-all", 8), ("length-quality-passed", 4)] {
        let chart = html
            .split(&format!("id=\"{id}-caption\""))
            .nth(1)
            .unwrap()
            .split("</figure>")
            .next()
            .unwrap();
        let counts: Vec<_> = chart
            .split("data-read-count=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap().parse::<u64>().unwrap())
            .collect();
        assert_eq!(counts.len(), 21 * 26);
        assert_eq!(counts.iter().sum::<u64>(), expected);
    }
    assert!(html.contains("Mean read length by channel"));
    assert!(html.contains("value=\"meanlength\""));
    assert!(html.contains("data-meanlength=\"15\" data-passedlength=\"20\""));
    for forbidden in ["https://", "http://", "fetch(", "@import", "<link"] {
        assert!(!html.contains(forbidden));
    }
    directory.analyze(&data);
    assert_eq!(
        json_bytes,
        fs::read(directory.0.join("output/report.json")).unwrap()
    );
    assert_eq!(html, directory.html());
}

#[test]
fn tabular_css_bars_preserve_exact_values_and_zero_half_maximum_proportions() {
    let directory = TestDirectory::new();
    let data = format!(
        "{HEADER}{}{}{}",
        ROW,
        ROW.replace("\t10\t", "\t20\t")
            .replace("\t1\t1\t", "\t2\t7\t"),
        ROW.replace("\t10\t", "\t0\t")
            .replace("\t1\t1\t", "\t3\t12\t")
    );
    directory.analyze(&data);
    let html = directory.html();
    let table = html
        .split("id=\"channel-mux-table\"")
        .nth(1)
        .unwrap()
        .split("</table>")
        .next()
        .unwrap();
    for (number, percent) in [(10, "50.00"), (20, "100.00"), (0, "0.00")] {
        assert!(table.contains(&format!("<span class=\"record-count\">{number}</span><span class=\"table-bar\" style=\"--bar-width:{percent}%\" aria-hidden=\"true\"></span>")));
    }
    assert!(!html.contains("class=\"inline-bar\""));
    assert!(!table.contains("<svg"));
    let empty = TestDirectory::new();
    empty.analyze(HEADER);
    assert!(!empty.html().contains("<span class=\"table-bar\""));
}

#[test]
fn table_controls_preserve_pagination_behavior_when_node_is_available() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/report_tables.cjs");
    match Command::new("node").arg(script).output() {
        Ok(output) => assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Optional table behavior check skipped: Node.js is unavailable");
        }
        Err(error) => panic!("could not run table behavior check: {error}"),
    }
}

#[test]
fn print_omission_is_explicit_and_both_large_tables_keep_complete_noscript_data() {
    let directory = TestDirectory::new();
    directory.analyze(&format!("{HEADER}{ROW}"));
    let html = directory.html();
    assert!(html.contains("class=\"print-only notice\">Detailed channel and channel × mux tables are omitted from print."));
    let print_css = html
        .split("@media print")
        .nth(1)
        .unwrap()
        .split("</style>")
        .next()
        .unwrap();
    assert!(print_css.contains(".detail-table {\n        display:none!important;"));
    assert!(print_css.contains(".print-only {\n        display:block;"));
    assert!(!print_css.contains("display:table-row!important"));
    for prefix in ["channel", "channel-mux"] {
        let before_table = html
            .split(&format!("<table id=\"{prefix}-table\""))
            .next()
            .unwrap();
        assert!(
            before_table
                .rsplit("<div ")
                .next()
                .unwrap()
                .contains("detail-table")
        );
        let table = html
            .split(&format!("<table id=\"{prefix}-table\""))
            .nth(1)
            .unwrap()
            .split("</table>")
            .next()
            .unwrap();
        assert_eq!(table.matches("<tr data-channel=").count(), 1);
        assert!(!table.contains("<tr hidden"));
    }
}

#[test]
fn large_synthetic_report_retains_all_source_rows_and_no_table_svg_microbars() {
    let directory = TestDirectory::new();
    let mut data = Vec::new();
    large_tables::write(&mut data).unwrap();
    let report = directory.analyze_with_args(
        std::str::from_utf8(&data).unwrap(),
        &["--flowcell-layout", "promethion-3000"],
    );
    assert_eq!(report["metrics"]["total_records"], 20000);
    assert_eq!(report["channels"].as_array().unwrap().len(), 3000);
    assert_eq!(
        report["diagnostics"]["channel_mux"]
            .as_array()
            .unwrap()
            .len(),
        10000
    );
    let html = directory.html();
    for (prefix, expected) in [("channel", 3000), ("channel-mux", 10000)] {
        let table = html
            .split(&format!("id=\"{prefix}-table\""))
            .nth(1)
            .unwrap()
            .split("</table>")
            .next()
            .unwrap();
        assert_eq!(table.matches("<tr data-channel=").count(), expected);
        assert!(!table.contains("<svg"));
    }
    assert!(!html.contains("class=\"inline-bar\""));
    assert_eq!(html.matches("class=\"heatmap-cell\"").count(), 9000);
    assert!(
        // Ten added scientific comparison charts; never one SVG per table row.
        html.matches("<svg ").count() <= 40,
        "SVG count must follow scientific charts, not table rows"
    );
    assert!(
        html.len() < 11_000_000,
        "unexpected synthetic HTML growth: {} bytes",
        html.len()
    );
    for resource in [
        "https://", "http://", "fetch(", "@import", "<link", "<iframe",
    ] {
        assert!(!html.contains(resource), "external resource: {resource}");
    }
}

#[test]
fn q_cutoff_is_inclusive_exact_and_independent_of_native_filtering() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("parity.tsv")).unwrap();
    let report = directory.analyze(&data);
    let c = &report["comparison"];
    assert_eq!(c["qscore_cutoff"], 7.0);
    assert_eq!(c["native_filtering_available"], true);
    assert_eq!(c["native_unclassified_reads"], 0);
    assert_eq!(report["metrics"]["total_records"], 8);
    assert_eq!(report["metrics"]["basecalled_reads"], 6);
    assert_eq!(report["metrics"]["total_yield_bp"], 380010);
    assert_eq!(report["metrics"]["passed_reads"], 2);
    assert_eq!(report["metrics"]["passed_yield_bp"], 120000);
    assert_eq!(
        report["diagnostics"]["passed_read_statistics"]["n50_bp"],
        100000
    );
    assert_eq!(
        c["qscore_selected"],
        json!({
            "read_count":5,"total_yield_bp":360010,"mean_read_length_bp":72002.0,
            "median_read_length_bp":50000.0,"n50_bp":200000,
            "min_read_length_bp":10,"max_read_length_bp":200000,
            "mean_qscore":8.8,"median_qscore":9.0
        })
    );
    let sum = |key: &str, field: &str| {
        c[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b[field].as_u64().unwrap())
            .sum::<u64>()
    };
    assert_eq!(sum("length_histogram", "read_count"), 5);
    assert_eq!(sum("length_histogram", "yield_bp"), 360010);
    assert_eq!(sum("quality_histogram", "read_count"), 5);
    assert_eq!(sum("time_bins", "basecalled_reads"), 5);
    assert_eq!(sum("time_bins", "yield_bp"), 360010);
    let time = &c["time_bins"];
    assert_eq!(time[0]["mean_read_length_bp"], 10.0);
    assert_eq!(time[0]["mean_qscore"], 9.0);
    assert!((time[1]["mean_read_length_bp"].as_f64().unwrap() - 350000.0 / 3.0).abs() < 1e-9);
    assert!((time[1]["mean_qscore"].as_f64().unwrap() - 28.0 / 3.0).abs() < 1e-12);
    assert_eq!(time[3]["basecalled_reads"], 0);
    assert!(time[3]["mean_read_length_bp"].is_null());
    assert!(time[3]["mean_qscore"].is_null());
    assert_eq!(time[3]["cumulative_yield_bp"], 360010);
    let q9 = directory.analyze_with_args(&data, &["--qscore-cutoff", "9"]);
    assert_eq!(q9["comparison"]["qscore_selected"]["read_count"], 3);
    assert_eq!(
        q9["comparison"]["qscore_selected"]["total_yield_bp"],
        300010
    );
    for key in ["metrics", "quality", "channels", "time_bins", "diagnostics"] {
        assert_eq!(report[key], q9[key], "cutoff changed native/all {key}");
    }
    let q0 = directory.analyze_with_args(&data, &["-q", "0"]);
    assert_eq!(q0["comparison"]["qscore_selected"]["read_count"], 6);
    let empty = directory.analyze_with_args(&data, &["--qscore-cutoff", "100"]);
    assert_eq!(empty["comparison"]["qscore_selected"]["read_count"], 0);
    for field in [
        "mean_read_length_bp",
        "median_read_length_bp",
        "n50_bp",
        "mean_qscore",
        "median_qscore",
    ] {
        assert!(empty["comparison"]["qscore_selected"][field].is_null());
    }
}

#[test]
fn missing_native_flag_is_unavailable_and_never_inferred_from_qscore() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("parity.tsv")).unwrap();
    let data = data
        .lines()
        .map(|line| {
            line.split('\t')
                .enumerate()
                .filter(|(i, _)| *i != 5)
                .map(|(_, v)| v)
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let report = directory.analyze(&data);
    assert_eq!(report["comparison"]["native_filtering_available"], false);
    assert_eq!(report["comparison"]["native_unclassified_reads"], 6);
    assert!(
        report["diagnostics"]["termination_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["passed_fraction_of_basecalled"].is_null())
    );
    assert_eq!(report["metrics"]["passed_reads"], 0);
    assert_eq!(report["metrics"]["failed_reads"], 0);
    assert_eq!(report["comparison"]["qscore_selected"]["read_count"], 5);
    let html = directory.html();
    assert!(html.contains("Native filtering unavailable"));
    assert!(html.contains("data-metric=\"pass_rate_percent\" data-value=\"\""));
    assert!(!html.contains("id=\"filter-reads-caption\""));
}

#[test]
fn invalid_qscore_cutoffs_are_actionable() {
    let directory = TestDirectory::new();
    for q in ["NaN", "inf", "-inf", "text"] {
        let result = directory.run_with_args(&fixture("parity.tsv"), &["--qscore-cutoff", q]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("qscore-cutoff"));
    }
}

#[test]
fn exact_channel_medians_include_explicit_summary_and_basecalled_denominators() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("parity.tsv")).unwrap();
    let report = directory.analyze(&data);
    let channels = report["comparison"]["channels"].as_array().unwrap();
    assert_eq!(channels[0]["channel"], 1);
    assert_eq!(
        channels[0]["all_summary_records"],
        json!({"read_count":4,"total_yield_bp":30010,"mean_read_length_bp":7502.5,"median_read_length_bp":5005.0})
    );
    assert_eq!(
        channels[0]["all_basecalled_reads"]["median_read_length_bp"],
        10000.0
    );
    assert_eq!(
        channels[0]["qscore_selected_reads"]["median_read_length_bp"],
        5005.0
    );
    assert_eq!(
        channels[1]["all_summary_records"]["median_read_length_bp"],
        100000.0
    );
    assert_eq!(
        channels[2]["all_summary_records"]["median_read_length_bp"],
        0.0
    );
    assert_eq!(
        channels[2]["all_summary_records"]["mean_read_length_bp"],
        0.0
    );
    assert!(channels[2]["all_basecalled_reads"]["median_read_length_bp"].is_null());
    assert!(channels[2]["qscore_selected_reads"]["median_read_length_bp"].is_null());
    assert_eq!(
        channels
            .iter()
            .map(|c| c["all_summary_records"]["read_count"].as_u64().unwrap())
            .sum::<u64>(),
        8
    );
    for (population, expected_reads, expected_yield) in [
        ("all_basecalled_reads", 6, 380010),
        ("qscore_selected_reads", 5, 360010),
    ] {
        assert_eq!(
            channels
                .iter()
                .map(|c| c[population]["read_count"].as_u64().unwrap())
                .sum::<u64>(),
            expected_reads
        );
        assert_eq!(
            channels
                .iter()
                .map(|c| c[population]["total_yield_bp"].as_u64().unwrap())
                .sum::<u64>(),
            expected_yield
        );
    }
    let distributions = report["comparison"]["channel_distributions"]
        .as_array()
        .unwrap();
    assert_eq!(distributions.len(), 4);
    for distribution in distributions {
        let bins = distribution["bins"].as_array().unwrap();
        assert_eq!(bins.len(), 30);
        assert_eq!(
            bins.iter()
                .map(|b| b["all_summary_channels"].as_u64().unwrap())
                .sum::<u64>(),
            3
        );
        assert_eq!(
            bins.iter()
                .map(|b| b["qscore_selected_channels"].as_u64().unwrap())
                .sum::<u64>(),
            2
        );
    }
    let html = directory.html();
    for id in [
        "total_yield_bp",
        "read_count",
        "mean_read_length_bp",
        "median_read_length_bp",
    ] {
        assert!(html.contains(&format!("id=\"channel-summary-{id}-caption\"")));
    }
    assert!(html.contains("data-medianlength=\"10000\" data-qreads=\"2\""));
}

#[test]
fn exact_yield_thresholds_curve_and_ultralong_are_consistent() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("parity.tsv")).unwrap();
    let report = directory.analyze(&data);
    let analysis = &report["comparison"]["yield_by_minimum_length"];
    let thresholds = analysis["thresholds"].as_array().unwrap();
    assert_eq!(
        thresholds
            .iter()
            .map(|t| t["minimum_length_bp"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![10000, 20000, 50000, 100000, 200000, 500000, 1000000]
    );
    assert_eq!(
        thresholds[0],
        json!({"minimum_length_bp":10000,"all_read_count":5,"all_yield_bp":380000,"qscore_selected_read_count":4,"qscore_selected_yield_bp":360000})
    );
    assert_eq!(thresholds[1]["all_yield_bp"], 370000);
    assert_eq!(thresholds[1]["qscore_selected_yield_bp"], 350000);
    assert_eq!(thresholds[4]["all_read_count"], 1);
    assert_eq!(thresholds[5]["all_read_count"], 0);
    let curve = analysis["curve"].as_array().unwrap();
    assert!(curve.len() <= 520);
    assert_eq!(curve[0]["minimum_length_bp"], 1);
    assert_eq!(
        curve[0]["all_yield_bp"],
        report["metrics"]["total_yield_bp"]
    );
    assert_eq!(curve.last().unwrap()["all_yield_bp"], 0);
    for pair in curve.windows(2) {
        assert!(pair[0]["minimum_length_bp"].as_u64() < pair[1]["minimum_length_bp"].as_u64());
        assert!(pair[0]["all_yield_bp"].as_u64() >= pair[1]["all_yield_bp"].as_u64());
        assert!(
            pair[0]["qscore_selected_yield_bp"].as_u64()
                >= pair[1]["qscore_selected_yield_bp"].as_u64()
        );
    }
    assert_eq!(analysis["ultralong_all"]["read_count"], 6);
    assert_eq!(analysis["ultralong_all"]["total_yield_bp"], 380010);
    assert_eq!(analysis["ultralong_qscore_selected"]["read_count"], 5);
    for (lengths, expected_count, expected_yield) in [
        (vec![100000, 50000, 50000], 2, 150000),
        (vec![100000, 100000], 2, 200000),
        (vec![99999, 50000], 0, 0),
        (vec![100000], 1, 100000),
        (vec![], 0, 0),
    ] {
        let mut input = HEADER.to_owned();
        for length in lengths {
            input += &format!("fictional\t1\t1\t0\t1\tFALSE\t{length}\t7\tnormal\n");
        }
        let report = directory.analyze(&input);
        let ultra = &report["comparison"]["yield_by_minimum_length"]["ultralong_all"];
        assert_eq!(ultra["read_count"], expected_count);
        assert_eq!(ultra["total_yield_bp"], expected_yield);
    }
}

#[test]
fn new_comparisons_handle_empty_zero_only_and_extreme_inputs_offline() {
    let directory = TestDirectory::new();
    for input in [
        HEADER.to_owned(),
        format!("{HEADER}fictional\t1\t1\t0\t1\tTRUE\t0\t0\t<script>\n"),
    ] {
        let report = directory.analyze_with_args(&input, &["-q", "0"]);
        assert_eq!(report["comparison"]["qscore_selected"]["read_count"], 0);
        assert!(report["comparison"]["qscore_selected"]["median_read_length_bp"].is_null());
        let html = directory.html();
        assert!(html.contains("Q ≥ 0"));
        assert!(!html.contains("<script>\n</th>"));
        assert!(!html.contains("https://"));
    }
    let input = format!(
        "{HEADER}fictional\t1\t1\t0\t1\tFALSE\t18446744073709551615\t1.7976931348623157e308\t&lt;\n"
    );
    let report = directory.analyze_with_args(&input, &["-q", "1.7976931348623157e308"]);
    assert_eq!(report["comparison"]["qscore_selected"]["read_count"], 1);
    assert_eq!(
        report["comparison"]["yield_by_minimum_length"]["thresholds"][6]["all_yield_bp"],
        u64::MAX
    );
    assert!(!directory.html().contains("Infinity"));
}

#[test]
fn parity_json_html_are_deterministic_and_use_the_same_exact_subset_values() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("parity.tsv")).unwrap();
    let report = directory.analyze(&data);
    let json_before = fs::read(directory.0.join("output/report.json")).unwrap();
    let html = directory.html();
    for (key, expected) in [
        ("read_count", "5"),
        ("total_yield_bp", "360010"),
        ("median_read_length_bp", "50000"),
        ("n50_bp", "200000"),
        ("mean_qscore", "8.8"),
    ] {
        assert!(html.contains(&format!(
            "data-metric=\"qscore_selected_{key}\" data-value=\"{expected}\""
        )));
    }
    assert_eq!(report, directory.analyze(&data));
    assert_eq!(
        json_before,
        fs::read(directory.0.join("output/report.json")).unwrap()
    );
    assert_eq!(html, directory.html());
}

#[test]
fn q_cutoff_even_and_odd_exact_statistics_preserve_n50_half_convention() {
    let directory = TestDirectory::new();
    let mut data = HEADER.to_owned();
    for (length, q, passed) in [
        (10, 7, "FALSE"),
        (20, 8, "TRUE"),
        (30, 9, "FALSE"),
        (40, 10, "TRUE"),
    ] {
        data += &format!("fictional\t1\t1\t0\t1\t{passed}\t{length}\t{q}\tnormal\n");
    }
    for (cutoff, reads, median, n50, median_q) in [
        ("7", 4, 25.0, 30, 8.5),
        ("8", 3, 30.0, 30, 9.0),
        ("9", 2, 35.0, 40, 9.5),
    ] {
        let report = directory.analyze_with_args(&data, &["-q", cutoff]);
        let q = &report["comparison"]["qscore_selected"];
        assert_eq!(q["read_count"], reads);
        assert_eq!(q["median_read_length_bp"], median);
        assert_eq!(q["n50_bp"], n50);
        assert_eq!(q["median_qscore"], median_q);
    }
    let mut input = HEADER.to_owned();
    for length in [100, 50, 50] {
        input += &format!("fictional\t1\t1\t0\t1\tFALSE\t{length}\t7\tnormal\n");
    }
    let report = directory.analyze(&input);
    assert_eq!(report["metrics"]["n50_bp"], 100); // Upstream's strict-half version returns 50.
    assert_eq!(report["comparison"]["qscore_selected"]["n50_bp"], 100);
}

#[test]
fn every_display_query_uses_exact_lengths_including_ties() {
    let directory = TestDirectory::new();
    let lengths = [
        (1, 6),
        (10000, 7),
        (10000, 9),
        (20000, 12),
        (999999, 0),
        (1000000, 9),
        (1000000, 5),
        (12345678, 10),
    ];
    let mut input = HEADER.to_owned();
    for (length, q) in lengths {
        input += &format!("fictional\t1\t1\t0\t1\tFALSE\t{length}\t{q}\tnormal\n");
    }
    let report = directory.analyze(&input);
    let analysis = &report["comparison"]["yield_by_minimum_length"];
    for query in analysis["curve"]
        .as_array()
        .unwrap()
        .iter()
        .chain(analysis["thresholds"].as_array().unwrap())
    {
        let threshold = query["minimum_length_bp"].as_u64().unwrap();
        let all: Vec<_> = lengths
            .iter()
            .filter(|(length, _)| *length >= threshold)
            .collect();
        let selected: Vec<_> = all.iter().filter(|(_, q)| *q >= 7).collect();
        assert_eq!(query["all_read_count"], all.len());
        assert_eq!(
            query["all_yield_bp"],
            all.iter().map(|(length, _)| *length).sum::<u64>()
        );
        assert_eq!(query["qscore_selected_read_count"], selected.len());
        assert_eq!(
            query["qscore_selected_yield_bp"],
            selected.iter().map(|(length, _)| *length).sum::<u64>()
        );
    }
}

#[test]
fn decimal_cutoff_labels_preserve_selection_precision() {
    let directory = TestDirectory::new();
    let input = format!(
        "{HEADER}fictional\t1\t1\t0\t1\tTRUE\t10\t7.12345599\tnormal\nfictional\t1\t1\t0\t1\tFALSE\t20\t7.123456\tnormal\n"
    );
    let report = directory.analyze_with_args(&input, &["-q", "7.123456"]);
    assert_eq!(report["comparison"]["qscore_selected"]["read_count"], 1);
    assert_eq!(
        report["comparison"]["qscore_selected"]["total_yield_bp"],
        20
    );
    assert!(directory.html().contains("Q ≥ 7.123456"));
}

#[test]
fn signed_zero_quality_retains_original_exact_median_serialization() {
    let directory = TestDirectory::new();
    directory.analyze_with_args(
        &format!("{HEADER}fictional\t1\t1\t0\t1\tTRUE\t10\t-0.0\tnormal\n"),
        &["-q", "0"],
    );
    let raw = fs::read_to_string(directory.0.join("output/report.json")).unwrap();
    assert_eq!(raw.matches("\"median_qscore\": -0.0").count(), 3);
}

fn run_collection(directory: &TestDirectory, inputs: &[&Path], args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .arg("analyze")
        .args(inputs)
        .args(args)
        .arg("--output")
        .arg(directory.0.join("output"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&fs::read(directory.0.join("output/combined/report.json")).unwrap())
        .unwrap()
}

#[test]
fn multiple_runs_chunks_and_duplicate_paths_have_exact_pooled_statistics() {
    let directory = TestDirectory::new();
    let input = directory.0.join("summaries with spaces");
    fs::create_dir_all(input.join("nested")).unwrap();
    let row = |run: &str, length: u64, q: u64| {
        format!("{run}\t1\t1\t0\t1\tTRUE\t{length}\t{q}\tnormal\n")
    };
    let a = input.join("sequencing_summary_a.tsv");
    fs::write(
        &a,
        format!("{HEADER}{}{}", row("a", 10, 6), row("a", 20, 7)),
    )
    .unwrap();
    fs::write(
        input.join("nested/sequencing_summary_chunk.tsv"),
        format!("{HEADER}{}", row("a", 30, 9)),
    )
    .unwrap();
    fs::write(
        input.join("sequencing_summary_b.txt"),
        format!("{HEADER}{}", row("b", 1000, 12)),
    )
    .unwrap();
    fs::write(input.join("unrelated.tsv"), "invalid unrelated file").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&input, input.join("nested/cycle")).unwrap();
        std::os::unix::fs::symlink(&a, input.join("sequencing_summary_alias.txt")).unwrap();
        fs::hard_link(&a, input.join("sequencing_summary_hardlink.txt")).unwrap();
    }
    let report = run_collection(&directory, &[&input, &a], &[]);
    assert_eq!(report["metrics"]["total_records"], 4);
    assert_eq!(report["metrics"]["total_yield_bp"], 1060);
    assert_eq!(report["metrics"]["n50_bp"], 1000);
    assert_eq!(report["metrics"]["median_read_length_bp"], 25.0);
    assert_eq!(report["metrics"]["mean_read_length_bp"], 265.0);
    assert_eq!(report["quality"]["median_qscore"], 8.0);
    assert_eq!(report["comparison"]["qscore_selected"]["read_count"], 3);
    assert_eq!(
        report["comparison"]["qscore_selected"]["median_read_length_bp"],
        30.0
    );
    assert_eq!(report["collection"]["runs"].as_array().unwrap().len(), 2);
    assert!(report["channels"].as_array().unwrap().is_empty());
    let runs = report["collection"]["runs"].as_array().unwrap();
    let chunked = runs.iter().find(|r| r["run_id"] == "a").unwrap();
    assert_eq!(chunked["source"]["input_files"], 2);
    assert_eq!(chunked["metrics"]["total_yield_bp"], 60);
    assert_eq!(chunked["metrics"]["median_read_length_bp"], 20.0);
    for id in ["run_001", "run_002"] {
        assert!(
            directory
                .0
                .join(format!("output/runs/{id}/report.html"))
                .exists()
        );
    }
    let html = fs::read_to_string(directory.0.join("output/combined/report.html")).unwrap();
    assert!(html.contains("Pooled runs / sources"));
    for family in [
        "combined-distribution-false-false",
        "combined-distribution-true-true",
        "combined-yield-time",
        "combined-length-time",
        "combined-quality-time",
        "combined-yield-length",
    ] {
        assert!(html.contains(family));
    }
    assert!(!html.contains("https://"));
    let before = fs::read(directory.0.join("output/combined/report.json")).unwrap();
    assert_eq!(report, run_collection(&directory, &[&input], &[]));
    assert_eq!(
        before,
        fs::read(directory.0.join("output/combined/report.json")).unwrap()
    );
}

#[test]
fn combined_only_skips_individual_outputs_and_explicit_grouping_rejects_conflicts() {
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    fs::write(&a, format!("{HEADER}{ROW}")).unwrap();
    fs::write(
        &b,
        format!("{HEADER}{}", ROW.replace("synthetic-run", "other-run")),
    )
    .unwrap();
    let report = run_collection(&directory, &[&a, &b], &["--combined-only"]);
    assert_eq!(report["metrics"]["basecalled_reads"], 2);
    assert!(!directory.0.join("output/runs").exists());
    let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .arg("analyze")
        .args([&a, &b])
        .arg("--group-inputs")
        .arg("--output")
        .arg(directory.0.join("invalid"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("different known run_id"));
}

#[test]
fn flowcell_metadata_conflicts_are_rejected_without_invented_ids() {
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    fs::write(
        &a,
        format!(
            "{}\tflow_cell_id\n{}\tfictional-cell-a\n",
            HEADER.trim_end(),
            ROW.trim_end()
        ),
    )
    .unwrap();
    fs::write(
        &b,
        format!(
            "{}\tflow_cell_id\n{}\tfictional-cell-b\n",
            HEADER.trim_end(),
            ROW.trim_end()
        ),
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .arg("analyze")
        .args([a, b])
        .arg("--output")
        .arg(directory.0.join("invalid"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("conflicting flow_cell_id"));
}

#[test]
fn mixed_run_single_files_require_explicit_splitting() {
    let directory = TestDirectory::new();
    let file = directory.0.join("mixed.tsv");
    fs::write(
        &file,
        format!("{HEADER}{ROW}{}", ROW.replace("synthetic-run", "second")),
    )
    .unwrap();
    assert!(!directory.run(&file).status.success());
    let report = run_collection(&directory, &[&file], &["--split-runs"]);
    assert_eq!(report["collection"]["runs"].as_array().unwrap().len(), 2);
    assert_eq!(report["metrics"]["total_yield_bp"], 20);
}

#[test]
fn gaussian_density_matches_independent_direct_kernel_evaluation() {
    let directory = TestDirectory::new();
    let mut data = HEADER.to_owned();
    let qscores = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
    for q in qscores {
        data += &format!("fictional\t1\t1\t0\t1\tTRUE\t10\t{q}\tnormal\n");
    }
    let report = directory.analyze(&data);
    let estimate = &report["distributions"]["qscore"];
    let bandwidth = 0.9 * 2.5_f64.sqrt().min(2.0 / 1.34) * 5.0_f64.powf(-0.2);
    assert!((estimate["bandwidth"].as_f64().unwrap() - bandwidth).abs() < 1e-12);
    assert_eq!(estimate["observations"], 5);
    let points = estimate["points"].as_array().unwrap();
    assert_eq!(points.len(), 512);
    let reference: Vec<f64> = points
        .iter()
        .map(|p| {
            let x = p["x"].as_f64().unwrap();
            qscores
                .iter()
                .map(|q| {
                    (-0.5 * ((x - q) / bandwidth).powi(2)).exp()
                        / (2.0 * std::f64::consts::PI).sqrt()
                        / bandwidth
                        / 5.0
                })
                .sum()
        })
        .collect();
    let peak = reference.iter().copied().reduce(f64::max).unwrap();
    for (p, reference) in points.iter().zip(reference) {
        assert!((p["density"].as_f64().unwrap() - reference).abs() < peak * 0.005);
    }
}

#[test]
fn minimal_legacy_inputs_keep_missing_metadata_unavailable() {
    let directory = TestDirectory::new();
    let input = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n1\t0\t10\t7\n1\t900\t20\t9\n2\t0\t0\t0\n";
    let report = directory.analyze(input);
    assert!(report["run"]["run_id"].is_null());
    assert_eq!(report["metrics"]["total_records"], 3);
    assert_eq!(report["metrics"]["total_yield_bp"], 30);
    assert_eq!(report["comparison"]["native_unclassified_reads"], 2);
    assert_eq!(report["comparison"]["qscore_selected"]["read_count"], 2);
    assert!(report["muxes"].as_array().unwrap().is_empty());
    assert!(report["end_reasons"].as_array().unwrap().is_empty());
    assert_eq!(report["input"]["schemas"], json!(["legacy"]));
    assert_eq!(report["input"]["read_representation"], "template");
    assert_invalid_with_args(
        input,
        "missing required column",
        &["--input-schema", "modern"],
    );
}

#[test]
fn legacy_2d_substitution_is_consistent_and_never_falls_back_per_row() {
    let directory = TestDirectory::new();
    let input = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\tsequence_length_2d\tmean_qscore_2d\tpasses_filtering\ttemplate_duration\tnum_events_template\n1\t0\t100\t20\t10\t7\tTRUE\t1\t200\n1\t900\t200\t30\t30\t9\tFALSE\t1\t400\n";
    let report = directory.analyze(input);
    assert_eq!(report["metrics"]["total_yield_bp"], 40);
    assert_eq!(report["quality"]["mean_qscore"], 8.0);
    assert_eq!(report["input"]["read_representation"], "2d");
    assert_eq!(report["comparison"]["native_unclassified_reads"], 2);
    assert!(report["diagnostics"]["sequencing_speed"].is_null());
    let template = directory.analyze_with_args(input, &["--read-representation", "template"]);
    assert_eq!(template["metrics"]["total_yield_bp"], 300);
    assert_eq!(template["metrics"]["passed_reads"], 1);
    assert_invalid(
        &input.replace("\t10\t7\t", "\tNA\t7\t"),
        "sequence_length_2d",
    );
    assert_invalid(
        "channel\tstart_time\tsequence_length_2d\n1\t0\t10\n",
        "mean_qscore_2d",
    );
    assert_invalid(
        "channel\tstart_time\tmean_qscore_2d\n1\t0\t10\n",
        "sequence_length_2d",
    );
}

#[test]
fn direct_rna_controls_are_excluded_and_counted_separately() {
    let directory = TestDirectory::new();
    let data = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\tcalibration_strand_genome_template\n1\t0\t100000\t20\tYHR174W\n1\t0\t10\t7\tno_match\n1\t0\t20\t8\tYHR174W-like\n";
    let report = directory.analyze(data);
    assert_eq!(report["input"]["input_records"], 3);
    assert_eq!(report["input"]["excluded_control_records"], 1);
    assert_eq!(report["metrics"]["total_records"], 2);
    assert_eq!(report["metrics"]["total_yield_bp"], 30);
    assert!(
        directory
            .html()
            .contains("1 YHR174W control observations excluded from 3")
    );
    assert_invalid(
        &data.replace("100000\t20", "NA\t20"),
        "sequence_length_template",
    );
}

#[test]
fn unknown_rna_calibration_labels_are_retained_instead_of_assumed_controls() {
    // Pinned R subset(label != "YHR174W") also drops missing labels. Our
    // documented exact-name policy retains unknown observations explicitly.
    let report = TestDirectory::new().analyze(
        "channel\tstart_time\tsequence_length_template\tmean_qscore_template\tcalibration_strand_genome_template\n\
         1\t0\t10\t9\t\n\
         1\t1\t20\t9\tNA\n\
         1\t2\t30\t9\tno_match\n\
         1\t3\t40\t9\tYHR174W\n",
    );
    assert_eq!(report["input"]["input_records"], 4);
    assert_eq!(report["input"]["excluded_control_records"], 1);
    assert_eq!(report["metrics"]["total_records"], 3);
    assert_eq!(report["metrics"]["total_yield_bp"], 60);
}

#[test]
fn legacy_signed_qscores_are_preserved_with_histogram_conservation() {
    let directory = TestDirectory::new();
    let input = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n1\t0\t5\t-0.1\n1\t900\t10\t7\n";
    let report = directory.analyze(input);
    assert_eq!(report["input"]["negative_qscore_records"], 1);
    assert_eq!(report["quality"]["histogram"][0]["read_count"], 1);
    assert_eq!(report["quality"]["histogram"][0]["upper_exclusive"], 0.0);
    assert!((report["quality"]["median_qscore"].as_f64().unwrap() - 3.45).abs() < 1e-12);
    assert_eq!(report["comparison"]["qscore_selected"]["read_count"], 1);
    assert_eq!(
        report["diagnostics"]["length_quality_histogram"]["all_read_counts"][0][2],
        1
    );
    assert_eq!(
        report["diagnostics"]["length_quality_histogram"]["all_read_counts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|r| r.as_array().unwrap())
            .map(|v| v.as_u64().unwrap())
            .sum::<u64>(),
        2
    );
    let html = directory.html();
    let joint = html
        .split("id=\"length-quality-all-caption\"")
        .nth(1)
        .unwrap()
        .split("</figure>")
        .next()
        .unwrap();
    assert!(joint.contains("&lt; 0"));
    assert!(joint.contains(">10</text>") && joint.contains(">≥ 50</text>"));
    assert!(!joint.contains(">8</text>") && !joint.contains("-1.798e308"));
    assert!(html.contains("below 0"));
    // The finite sentinel remains in canonical JSON; only its display changes.
    assert_eq!(
        report["diagnostics"]["length_quality_histogram"]["qscore_lower_bounds"][0],
        -f64::MAX
    );
    let extreme = input
        .replace("-0.1", "-1.7976931348623157e308")
        .replace("\t7\n", "\t1.7976931348623157e308\n");
    let report = directory.analyze(&extreme);
    assert_eq!(report["quality"]["mean_qscore"], 0.0);
    assert_eq!(report["quality"]["median_qscore"], 0.0);
}

#[test]
fn identity_free_sources_are_separate_unless_explicitly_grouped() {
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    let data = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n1\t0\t10\t9\n";
    fs::write(&a, data).unwrap();
    fs::write(&b, data).unwrap();
    let report = run_collection(&directory, &[&a, &b], &[]);
    assert_eq!(report["collection"]["runs"].as_array().unwrap().len(), 2);
    assert_eq!(
        report["collection"]["runs"][0]["source"]["grouping"],
        "unidentified_source"
    );
    assert!(report["collection"]["runs"][0]["run_id"].is_null());
    let report = run_collection(&directory, &[&a, &b], &["--group-inputs"]);
    assert_eq!(report["collection"]["runs"].as_array().unwrap().len(), 1);
    assert_eq!(report["metrics"]["total_yield_bp"], 20);
    let two_d = "channel\tstart_time\tsequence_length_2d\tmean_qscore_2d\n1\t0\t10\t9\n";
    fs::write(&b, two_d).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .arg("analyze")
        .args([a, b])
        .arg("--group-inputs")
        .arg("--output")
        .arg(directory.0.join("invalid"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("incompatible template/2D"));
}

#[test]
fn minion_physical_maps_and_exact_points_preserve_all_observations() {
    let directory = TestDirectory::new();
    let mut input = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\tnum_events_template\n".to_owned();
    for i in 0..100 {
        input += &format!(
            "1\t{}\t{}\t{}\t{}\n",
            i * 900,
            100 + i,
            5 + i % 10,
            (100 + i) * 2
        );
    }
    input += "512\t0\t20\t9\t40\n33\t0\t0\t0\t0\n";
    let report = directory.analyze_with_args(&input, &["--flowcell-layout", "minion-r9.5"]);
    assert_eq!(report["diagnostics"]["physical_layout"]["rows"], 32);
    assert_eq!(report["diagnostics"]["physical_layout"]["columns"], 16);
    assert!(report["diagnostics"]["minion_sample"].is_null());
    let view = &report["diagnostics"]["minion_read_visualization"];
    assert_eq!(view["eligible_reads"], 101);
    assert_eq!(view["zero_length_records"], 1);
    assert_eq!(view["points"].as_array().unwrap().len(), 101);
    assert_eq!(view["method"], "exact_points");
    assert_eq!(view["channels"][0]["eligible_reads"], 100);
    assert_eq!(view["channels"][2]["eligible_reads"], 1);
    assert_eq!(view["points"][0]["events_per_base"], 2.0);
    let html = directory.html();
    assert!(html.contains("MinION physical flowcell overview"));
    assert!(html.contains("data-channel=\"1\" data-row=\"0\" data-column=\"15\""));
    assert!(html.contains("data-channel=\"512\" data-row=\"7\" data-column=\"7\""));
    assert_eq!(html.matches("class=\"heatmap-cell\"").count(), 1024); // native flag unavailable; all + Q selected
    assert_eq!(
        report,
        directory.analyze_with_args(&input, &["--flowcell-layout", "minion-r9.5"])
    );
    assert_eq!(html, directory.html());
    assert_invalid_with_args(
        &input.replace("512\t0", "513\t0"),
        "expected 1–512",
        &["--flowcell-layout", "minion-r9.5"],
    );
    let no_map = directory.analyze(&input);
    assert!(no_map["diagnostics"]["minion_sample"].is_null());
}

#[test]
fn dense_minion_reports_embed_complete_offline_rasters_and_preserve_metrics() {
    let directory = TestDirectory::new();
    let header = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\tnum_events_template\n";
    let mut rows = Vec::new();
    for i in 0..6000 {
        rows.push(format!(
            "{}\t{}\t{}\t{}\t{}\n",
            i % 512 + 1,
            i % 4 * 900,
            100 + i % 7,
            5 + i % 9,
            if i % 3 == 0 {
                "NA"
            } else if i % 3 == 1 {
                "0"
            } else {
                "200"
            }
        ));
    }
    rows.push("512\t10000\t1000000\t100\t2000000\n".into());
    rows.push("33\t0\t1\t-3\t1\n".into());
    rows.push("1\t0\t0\t0\t0\n".into());
    let data = format!("{header}{}", rows.concat());
    let original = directory.analyze(&data);
    let report = directory.analyze_with_args(
        &data,
        &["--flowcell-layout", "minion-r9.5", "--format", "svg"],
    );
    for field in [
        "metrics",
        "quality",
        "comparison",
        "time_bins",
        "channels",
        "input",
        "distributions",
    ] {
        assert_eq!(report[field], original[field], "{field}");
    }
    let view = &report["diagnostics"]["minion_read_visualization"];
    assert_eq!(view["method"], "full_count_pixel_aggregation");
    assert_eq!(view["eligible_reads"], 6002);
    assert_eq!(view["zero_length_records"], 1);
    assert_eq!(view["missing_events_reads"], 2000);
    assert_eq!(view["zero_events_reads"], 2000);
    assert!(view["points"].as_array().unwrap().is_empty());
    for field in ["overview", "scatter"] {
        assert_eq!(view[field]["total_reads"], 6002);
        assert_eq!(
            view[field]["counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r[1].as_u64().unwrap())
                .sum::<u64>(),
            6002
        );
        assert!(
            view[field]["png_data_uri"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,iVBORw0KGgo")
        );
    }
    let html = directory.html();
    assert!(html.contains("img-src data:"));
    assert!(html.contains("Full-count pixel aggregation"));
    assert!(html.contains("natural-log color scale"));
    assert!(!html.contains("log10(1+"));
    assert!(!html.contains("reservoir"));
    assert!(!html.contains("NaN"));
    assert!(!html.contains("Infinity"));
    assert!(html.matches("<circle class=\"minion-point\"").count() <= 512 * 6 * 2);
    for href in html.split("href=\"").skip(1) {
        assert!(href.starts_with('#') || href.starts_with("data:image/png;base64,"));
    }
    let figure = fs::read_to_string(
        directory
            .0
            .join("output/figures/minion-flowcell-overview.svg"),
    )
    .unwrap();
    assert!(figure.contains("data:image/png;base64,"));
    assert!(figure.contains("href=\"#minion-atlas\""));
    rows.reverse();
    let reversed = format!("{header}{}", rows.concat());
    let reordered = directory.analyze_with_args(&reversed, &["--flowcell-layout", "minion-r9.5"]);
    assert_eq!(view, &reordered["diagnostics"]["minion_read_visualization"]);
    let combined = directory.0.join("combined");
    let input = directory.0.join("input.tsv");
    let output = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .args(["analyze"])
        .arg(input)
        .args([
            "--flowcell-layout",
            "minion-r9.5",
            "--combined-only",
            "--output",
        ])
        .arg(&combined)
        .output()
        .unwrap();
    assert!(output.status.success());
    let pooled: Value =
        serde_json::from_slice(&fs::read(combined.join("combined/report.json")).unwrap()).unwrap();
    assert!(
        pooled["diagnostics"]
            .get("minion_read_visualization")
            .is_none()
    );
}

// Read the emitted YAML mapping subset independently: keys/scalars use JSON
// quoting, which is valid YAML 1.2. Nested maps are determined by indentation.
fn yaml_mapping(text: &str) -> Value {
    fn parse(lines: &[&str], position: &mut usize, indent: usize) -> Value {
        let mut fields = serde_json::Map::new();
        while *position < lines.len() {
            let line = lines[*position];
            let spaces = line.len() - line.trim_start().len();
            if spaces < indent {
                break;
            }
            assert_eq!(spaces, indent);
            let (key, value) = line.trim_start().split_once(":").unwrap();
            let key: String = serde_json::from_str(key).unwrap();
            *position += 1;
            let value = if value.is_empty() {
                parse(lines, position, indent + 2)
            } else {
                serde_json::from_str(value.trim()).unwrap()
            };
            fields.insert(key, value);
        }
        Value::Object(fields)
    }
    let lines: Vec<_> = text.lines().filter(|l| *l != "---").collect();
    parse(&lines, &mut 0, 0)
}

#[test]
fn yaml_summary_projects_exact_json_metrics_without_conflating_native_passes() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("synthetic.tsv")).unwrap();
    let report = directory.analyze_with_args(&data, &["--yaml", "--qscore_cutoff", "15"]);
    let yaml = fs::read_to_string(directory.0.join("output/summary.yaml")).unwrap();
    let summary = yaml_mapping(&yaml);
    assert_eq!(summary["All reads"]["total.reads"], 4);
    assert_eq!(
        summary["All reads"]["total.bases"],
        report["metrics"]["total_yield_bp"]
    );
    assert_eq!(
        summary["All reads"]["N50.length"],
        report["metrics"]["n50_bp"]
    );
    assert_eq!(summary["All reads"]["median.length"], 25.0);
    assert_eq!(
        summary["All reads"]["mean.q"],
        report["quality"]["mean_qscore"]
    );
    assert_eq!(
        summary["Q>=15"]["total.bases"],
        report["comparison"]["qscore_selected"]["total_yield_bp"]
    );
    assert_eq!(summary["Q>=15"]["total.bases"], 50);
    assert_eq!(report["metrics"]["passed_yield_bp"], 40);
    assert_eq!(summary["summary.records"], 6);
    assert_eq!(summary["All reads"]["reads"][">10kb"], 0);
    assert_eq!(summary["All reads"]["reads"]["ultralong"], 0);
    assert!(summary["notes"].as_str().unwrap().contains("inclusive"));
    assert!(!yaml.contains("NaN"));
    let same = directory.analyze_with_args(&data, &["--yaml", "--qscore_cutoff", "15"]);
    assert_eq!(report, same);
    assert_eq!(
        yaml,
        fs::read_to_string(directory.0.join("output/summary.yaml")).unwrap()
    );
}

#[test]
fn yaml_handles_exact_thresholds_ultralong_and_nulls_with_safe_metadata() {
    let directory = TestDirectory::new();
    let data = format!(
        "{HEADER}{}
{}{}",
        ROW.trim_end()
            .replace("synthetic-run", "<tag>&\"quoted\"")
            .replace("\t10\t", "\t100000\t"),
        ROW.replace("\t10\t", "\t10000\t")
            .replace("synthetic-run", "<tag>&\"quoted\""),
        ROW.replace("synthetic-run", "<tag>&\"quoted\"")
            .replace("\t10\t", "\t0\t")
    );
    let report = directory.analyze_with_args(&data, &["--yaml"]);
    let summary =
        yaml_mapping(&fs::read_to_string(directory.0.join("output/summary.yaml")).unwrap());
    assert_eq!(summary["run_id"], "<tag>&\"quoted\"");
    assert_eq!(summary["All reads"]["reads"][">10kb"], 2);
    assert_eq!(summary["All reads"]["gigabases"][">10kb"], 110000.0 / 1e9);
    assert_eq!(
        summary["All reads"]["reads"]["ultralong"],
        report["comparison"]["yield_by_minimum_length"]["ultralong_all"]["read_count"]
    );
    directory.analyze_with_args(HEADER, &["--yaml"]);
    let empty = yaml_mapping(&fs::read_to_string(directory.0.join("output/summary.yaml")).unwrap());
    assert_eq!(empty["All reads"]["total.reads"], 0);
    assert!(empty["All reads"]["mean.q"].is_null());
    assert!(empty["Q>=7"]["N50.length"].is_null());
}

#[test]
fn svg_exports_are_self_contained_sized_and_annotated_from_existing_statistics() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("synthetic.tsv")).unwrap();
    let default = directory.analyze(&data);
    assert!(!directory.0.join("output/figures").exists());
    let report = directory.analyze_with_args(
        &data,
        &[
            "--format",
            "svg",
            "--smallfigures",
            "--add-stat",
            "--muxes",
            "0.25",
        ],
    );
    for key in [
        "metrics",
        "quality",
        "diagnostics",
        "comparison",
        "channels",
        "time_bins",
        "distributions",
    ] {
        assert_eq!(
            report[key], default[key],
            "presentation changed QC field {key}"
        );
    }
    let figures = directory.0.join("output/figures");
    let names: Vec<String> =
        serde_json::from_slice(&fs::read(figures.join("manifest.json")).unwrap()).unwrap();
    assert!(names.len() > 15);
    for name in &names {
        let svg = fs::read_to_string(figures.join(name)).unwrap();
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"120mm\""));
        assert!(svg.contains("<title id=\""));
        assert!(svg.ends_with("</svg>"));
        assert!(!svg.contains("<script"));
        assert!(!svg.contains("<link"));
        assert!(!svg.contains("href=\"https://"));
        assert!(!svg.contains("NaN"));
    }
    let length = fs::read_to_string(figures.join("length-counts.svg")).unwrap();
    assert!(length.contains("Mean length: 0.025 kb · N50: 0.03 kb"));
    assert!(length.contains("id=\"length-counts-caption\">Reads by length interval</title>"));
    let quality = fs::read_to_string(figures.join("quality-histogram.svg")).unwrap();
    assert!(quality.contains("Mean read Q-score: 15"));
    // Use the renderer's stable manifest, rather than guessing a figure title.
    let yield_svg = names
        .iter()
        .map(|n| fs::read_to_string(figures.join(n)).unwrap())
        .find(|s| s.contains("data-mux-reference-seconds"))
        .unwrap();
    assert!(yield_svg.contains("not a measured instrument event"));
    directory.analyze_with_args(&data, &["--format", "svg"]);
    assert!(
        fs::read_to_string(figures.join("length-counts.svg"))
            .unwrap()
            .contains("width=\"180mm\"")
    );
    assert!(!directory.html().contains("class=\"stat-annotation\""));
}

#[test]
fn export_failures_preserve_previous_reports_and_clean_all_temporary_outputs() {
    for blocked in ["summary.yaml", "figures"] {
        let directory = TestDirectory::new();
        directory.analyze(&format!("{HEADER}{ROW}"));
        let output = directory.0.join("output");
        let previous_json = fs::read(output.join("report.json")).unwrap();
        let previous_html = fs::read(output.join("report.html")).unwrap();
        fs::create_dir(output.join(blocked)).unwrap();
        if blocked == "figures" {
            fs::write(output.join("figures/length-counts.svg"), "previous SVG").unwrap();
            fs::create_dir(output.join("figures/quality-histogram.svg")).unwrap();
        }
        let input = directory.0.join("input.tsv");
        fs::write(
            &input,
            format!("{HEADER}{}", ROW.replace("\t10\t", "\t20\t")),
        )
        .unwrap();
        let result = directory.run_with_args(&input, &["--yaml", "--format", "svg"]);
        assert!(!result.status.success());
        assert_eq!(fs::read(output.join("report.json")).unwrap(), previous_json);
        assert_eq!(fs::read(output.join("report.html")).unwrap(), previous_html);
        for dir in [output.clone(), output.join("figures")] {
            if dir.is_dir() {
                for file in fs::read_dir(dir).unwrap() {
                    assert!(!file.unwrap().file_name().to_string_lossy().contains(".tmp"));
                }
            }
        }
        if blocked == "figures" {
            assert_eq!(
                fs::read_to_string(output.join("figures/length-counts.svg")).unwrap(),
                "previous SVG"
            );
            assert!(!output.join("summary.yaml").exists());
        }
    }
}

#[test]
fn configured_mux_markers_use_relative_time_and_can_be_disabled() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("synthetic.tsv")).unwrap();
    let report = directory.analyze_with_args(&data, &["--muxes", "0.25"]);
    assert_eq!(
        report["presentation"]["mux_markers_seconds"],
        json!([0.0, 900.0, 1800.0, 2700.0, 3600.0])
    );
    let html = directory.html();
    assert!(html.contains("class=\"mux-marker\""));
    assert!(html.contains("data-mux-reference-seconds=\"900\" x1=\"238.50\""));
    assert!(html.contains("not measured MinKNOW events"));
    directory.analyze_with_args(&data, &["--muxes", "0"]);
    assert!(!directory.html().contains("class=\"mux-marker\""));
    let empty = directory.analyze_with_args(HEADER, &["--muxes", "1"]);
    assert_eq!(empty["presentation"]["mux_markers_seconds"], json!([]));
    assert_invalid_with_args(&data, "more than 512 markers", &["--muxes", "0.000001"]);
    for bad in ["NaN", "inf", "-1"] {
        assert_invalid_with_args(&data, "error:", &["--muxes", bad]);
    }
}

#[test]
fn output_compatibility_aliases_and_explicit_unsupported_formats_are_actionable() {
    let directory = TestDirectory::new();
    let output = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .args(["analyze", "--input"])
        .arg(fixture("synthetic.tsv"))
        .args(["--outputdirectory"])
        .arg(directory.0.join("output"))
        .args([
            "--qscore_cutoff",
            "10",
            "--processors",
            "1",
            "--yaml",
            "--smallfigures",
            "--add-stat",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for args in [["--format", "webp"], ["--processors", "0"]] {
        let result = directory.run_with_args(&fixture("synthetic.tsv"), &args);
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains("invalid value"));
    }
}

#[test]
fn collection_exports_preserve_pooled_statistics_and_skip_individuals_when_requested() {
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    fs::write(&a, format!("{HEADER}{ROW}")).unwrap();
    fs::write(
        &b,
        format!(
            "{HEADER}{}",
            ROW.replace("synthetic-run", "different-run")
                .replace("\t10\t", "\t100\t")
        ),
    )
    .unwrap();
    let report = run_collection(
        &directory,
        &[&a, &b],
        &[
            "--combined-only",
            "--yaml",
            "--format",
            "svg",
            "--muxes",
            "1",
        ],
    );
    let combined = directory.0.join("output/combined");
    assert!(!directory.0.join("output/runs").exists());
    let yaml = yaml_mapping(&fs::read_to_string(combined.join("summary.yaml")).unwrap());
    assert_eq!(yaml["All reads"]["total.bases"], 110);
    assert_eq!(yaml["All reads"]["N50.length"], 100);
    assert_eq!(yaml["All reads"]["median.length"], 55.0);
    assert_eq!(yaml["All reads"]["N50.length"], report["metrics"]["n50_bp"]);
    let manifest = fs::read_to_string(combined.join("figures/manifest.json")).unwrap();
    assert!(manifest.contains("combined-yield-time-false.svg"));
    assert!(manifest.contains("combined-length-time-true.svg"));
    let html = fs::read_to_string(combined.join("report.html")).unwrap();
    assert!(html.contains("data-mux-reference-seconds=\"0\""));
}

#[test]
fn empty_exports_and_wide_minion_figures_keep_explicit_available_states() {
    let directory = TestDirectory::new();
    let empty = directory.analyze_with_args(HEADER, &["--yaml", "--format", "svg", "--add-stat"]);
    assert_eq!(empty["metrics"]["basecalled_reads"], 0);
    let manifest: Vec<String> = serde_json::from_slice(
        &fs::read(directory.0.join("output/figures/manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(manifest.is_empty());
    assert!(
        directory
            .html()
            .contains("No basecalled reads are available")
    );
    directory.analyze_with_args(
        &format!("{HEADER}{ROW}"),
        &[
            "--format",
            "svg",
            "--smallfigures",
            "--flowcell-layout",
            "minion-r9.5",
        ],
    );
    let figures = directory.0.join("output/figures");
    let overview = fs::read_to_string(figures.join("minion-flowcell-overview.svg")).unwrap();
    assert!(overview.contains("width=\"320mm\""));
    assert!(overview.contains("data-channel=\"1\""));
    let length_time = fs::read_to_string(figures.join("length-time.svg")).unwrap();
    assert!(length_time.contains(">All basecalled reads</text>"));
    assert!(length_time.contains(">Native passed reads</text>"));
    assert!(length_time.contains(">Q ≥ 7</text>"));
}

#[test]
fn explicitly_grouped_empty_chunks_do_not_invent_an_extra_run() {
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    fs::write(&a, format!("{HEADER}{ROW}")).unwrap();
    fs::write(&b, HEADER).unwrap();
    let report = run_collection(&directory, &[&a, &b], &["--group-inputs", "--yaml"]);
    assert_eq!(report["collection"]["runs"].as_array().unwrap().len(), 1);
    assert_eq!(report["collection"]["runs"][0]["source"]["input_files"], 2);
    let yaml = yaml_mapping(
        &fs::read_to_string(directory.0.join("output/runs/run_001/summary.yaml")).unwrap(),
    );
    assert_eq!(yaml["All reads"]["total.reads"], 1);
}

#[test]
fn optional_fits_keep_raw_metrics_and_populations_separate() {
    let directory = TestDirectory::new();
    let mut data =
        "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n".to_owned();
    for i in (0..99).rev() {
        data += &format!(
            "1\t{}\t{}\t{}\n",
            i * 71,
            if i == 0 { 0 } else { 100 + i * i },
            if i % 3 == 0 { 6 } else { 9 }
        );
    }
    let previous = directory.analyze(&data);
    assert!(previous.get("fitted_trends").is_none());
    let fitted = directory.analyze_with_args(&data, &["--minionqc-trends", "--muxes", "1"]);
    for key in [
        "metrics",
        "quality",
        "comparison",
        "time_bins",
        "diagnostics",
    ] {
        assert_eq!(previous[key], fitted[key]);
    }
    let curves = fitted["fitted_trends"]["single"].as_array().unwrap();
    assert_eq!(curves[0]["observations"], 99); // includes the zero-length record
    assert_eq!(curves[0]["points"].as_array().unwrap().len(), 80);
    assert_eq!(curves[2]["observations"], 66);
    assert!(curves[0]["method"].as_str().unwrap().starts_with("loess"));
    assert_eq!(
        curves[0]["confidence_interval"],
        "pointwise_95_percent_student_t"
    );
    for point in curves[0]["points"].as_array().unwrap() {
        assert!(point["standard_error"].as_f64().unwrap() >= 0.0);
        assert!(point["lower"].as_f64().unwrap() <= point["value"].as_f64().unwrap());
        assert!(point["upper"].as_f64().unwrap() >= point["value"].as_f64().unwrap());
    }
    let html = directory.html();
    assert!(html.contains("Provisional MinIONQC-style fit"));
    assert!(html.contains("pointwise 95% LOESS Student-t interval"));
    assert!(html.contains("class=\"mux-marker\""));
    assert!(!html.contains("NaN"));
    let same = directory.analyze_with_args(&data, &["--minionqc-trends", "--muxes", "1"]);
    assert_eq!(fitted, same);
    assert_eq!(html, directory.html());
}

#[test]
fn optional_gam_and_sparse_fits_are_bounded_and_unavailable_when_required() {
    let directory = TestDirectory::new();
    let mut data =
        "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n".to_owned();
    for i in 0..1200 {
        data += &format!("1\t{}\t{}\t{}\n", i * 10, 1000 + i % 100, 10 + i % 4);
    }
    let report = directory.analyze_with_args(&data, &["--minionqc-trends"]);
    for curve in report["fitted_trends"]["single"].as_array().unwrap() {
        assert!(curve["method"].as_str().unwrap().contains("REML"));
        assert_eq!(curve["points"].as_array().unwrap().len(), 80);
        assert_eq!(curve["exact_knots"], true);
        assert_eq!(curve["knots_hours"].as_array().unwrap().len(), 10);
        assert_eq!(
            curve["confidence_interval"],
            "pointwise_95_percent_bayesian_normal"
        );
        for point in curve["points"].as_array().unwrap() {
            let y = point["value"].as_f64().unwrap();
            assert!(point["standard_error"].as_f64().unwrap() >= 0.0);
            assert!(point["lower"].as_f64().unwrap() <= y);
            assert!(point["upper"].as_f64().unwrap() >= y);
        }
    }
    let html = directory.html();
    assert!(html.contains("<polygon class=\"fit-ribbon"));
    assert!(html.contains("pointwise 95%"));
    assert_eq!(
        report,
        directory.analyze_with_args(&data, &["--minionqc-trends"])
    );
    assert_eq!(html, directory.html());
    let empty = directory.analyze_with_args(HEADER, &["--minionqc-trends"]);
    assert!(
        empty["fitted_trends"]["single"][0]["points"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let sparse =
        directory.analyze_with_args(&format!("{HEADER}{ROW}{ROW}"), &["--minionqc-trends"]);
    assert!(sparse["fitted_trends"]["single"][0]["unavailable_reason"].is_string());
}

#[test]
fn gam_knots_are_exact_unique_time_quantiles_beyond_previous_sampling_limit() {
    let directory = TestDirectory::new();
    let mut data =
        "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n".to_owned();
    for i in 0..6000 {
        data += &format!(
            "1\t{}\t{}\t{}\n",
            i * 7 % 6000,
            1000 + i % 97,
            if i % 2 == 0 { 9.0 } else { 6.9 }
        );
    }
    let old = directory.analyze_with_args(&data, &["--qscore-cutoff", "9"]);
    let fitted = directory.analyze_with_args(&data, &["--qscore-cutoff", "9", "--minionqc-trends"]);
    for key in [
        "metrics",
        "quality",
        "comparison",
        "diagnostics",
        "time_bins",
    ] {
        assert_eq!(old[key], fitted[key]);
    }
    for curve in fitted["fitted_trends"]["single"].as_array().unwrap() {
        let end = if curve["population"] == "all_summary_records" {
            5999.0
        } else {
            5998.0
        };
        assert_eq!(curve["exact_knots"], true);
        for (i, knot) in curve["knots_hours"].as_array().unwrap().iter().enumerate() {
            assert!((knot.as_f64().unwrap() - end * i as f64 / 9.0 / 3600.0).abs() < 1e-12);
        }
    }
}

#[test]
fn reference_densities_keep_old_statistics_and_include_zero_q_only_in_named_population() {
    let directory = TestDirectory::new();
    let data = fs::read_to_string(fixture("synthetic.tsv")).unwrap();
    let old = directory.analyze(&data);
    let report =
        directory.analyze_with_args(&data, &["--minionqc-density", "--qscore-cutoff", "0"]);
    let normal = directory.analyze_with_args(&data, &["--qscore-cutoff", "0"]);
    for key in [
        "metrics",
        "quality",
        "comparison",
        "distributions",
        "diagnostics",
    ] {
        assert_eq!(normal[key], report[key]);
    }
    assert!(old.get("minionqc_density").is_none());
    let density = &report["minionqc_density"];
    assert_eq!(density["qscore"]["observations"], 6);
    assert_eq!(density["selected_qscore"]["observations"], 6);
    assert_eq!(density["length_log10"]["observations"], 4);
    assert_eq!(density["qscore"]["points"].as_array().unwrap().len(), 512);
    let empty = directory.analyze_with_args(HEADER, &["--minionqc-density"]);
    assert!(empty["minionqc_density"]["qscore"].is_null());
    let zero = directory.analyze_with_args(
        &format!(
            "{HEADER}{}{}",
            ROW.replace("\t10\t", "\t0\t"),
            ROW.replace("\t10\t", "\t0\t")
        ),
        &["--minionqc-density"],
    );
    assert!(zero["minionqc_density"]["qscore"].is_object());
    assert!(zero["quality"]["mean_qscore"].is_null());
}

#[test]
fn reference_densities_share_domains_across_unequal_sources_and_are_deterministic() {
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    fs::write(
        &a,
        format!("{HEADER}{ROW}{}", ROW.replace("\t10\t", "\t20\t")),
    )
    .unwrap();
    fs::write(
        &b,
        format!(
            "{HEADER}{}{}{}",
            ROW.replace("synthetic-run", "second")
                .replace("\t10\t", "\t1000000\t"),
            ROW.replace("synthetic-run", "second")
                .replace("\t12.5\t", "\t30\t"),
            ROW.replace("synthetic-run", "second")
                .replace("\t10\t", "\t0\t")
        ),
    )
    .unwrap();
    let report = run_collection(&directory, &[&a, &b], &["--minionqc-density"]);
    let runs = report["collection"]["runs"].as_array().unwrap();
    let first = &runs[0]["minionqc_density"]["length_log10"]["points"];
    let second = &runs[1]["minionqc_density"]["length_log10"]["points"];
    assert_eq!(first[0]["x"], 1.0);
    assert_eq!(second[511]["x"], 6.0);
    for (a, b) in first
        .as_array()
        .unwrap()
        .iter()
        .zip(second.as_array().unwrap())
    {
        assert_eq!(a["x"], b["x"]);
    }
    let html = fs::read_to_string(directory.0.join("output/combined/report.html")).unwrap();
    assert!(html.contains("R-grid density mode"));
    assert!(html.contains("All summary records"));
    assert!(!html.contains("NaN"));
    assert!(!html.contains("src=\"https://"));
    let same = run_collection(&directory, &[&a, &b], &["--minionqc-density"]);
    assert_eq!(report, same);
    assert_eq!(
        html,
        fs::read_to_string(directory.0.join("output/combined/report.html")).unwrap()
    );
}

#[test]
fn constant_reference_density_keeps_r_grid_and_displays_evaluated_points() {
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    fs::write(&a, format!("{HEADER}{ROW}{ROW}")).unwrap();
    fs::write(
        &b,
        format!(
            "{HEADER}{}{}",
            ROW.replace("synthetic-run", "second"),
            ROW.replace("synthetic-run", "second")
        ),
    )
    .unwrap();
    let report = run_collection(&directory, &[&a, &b], &["--minionqc-density"]);
    for run in report["collection"]["runs"].as_array().unwrap() {
        for field in [
            "length_log10",
            "qscore",
            "selected_length_log10",
            "selected_qscore",
        ] {
            let points = run["minionqc_density"][field]["points"].as_array().unwrap();
            assert_eq!(points.len(), 512);
            assert!(points.iter().all(|p| p["x"] == points[0]["x"]));
        }
    }
    let html = fs::read_to_string(directory.0.join("output/combined/report.html")).unwrap();
    assert!(html.contains("constant-density-point"));
    assert!(html.contains("Constant observed domain"));
    assert!(!html.contains("NaN"));
    assert!(!html.contains("src=\"https://"));
}

#[test]
fn optional_reference_views_keep_untrusted_metadata_escaped_and_offline() {
    let directory = TestDirectory::new();
    let data = format!(
        "{HEADER}{}",
        ROW.replace("synthetic-run", "<script>alert('fictional')</script>&")
    );
    directory.analyze_with_args(&data, &["--minionqc-trends", "--minionqc-density"]);
    let html = directory.html();
    assert_eq!(html.matches("<script>").count(), 1);
    assert!(html.contains("&lt;script&gt;alert(&#39;fictional&#39;)&lt;/script&gt;&amp;"));
    assert!(!html.contains("src=\"https://"));
    assert!(!html.contains("<link"));
}

#[test]
fn minion_visualizations_keep_legacy_event_and_control_semantics() {
    let directory = TestDirectory::new();
    let header = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\tnum_events_template\tcalibration_strand_genome_template\n";
    let data = format!(
        "{header}1\t0\t10\t-1\tNA\t\n2\t1\t20\t8\t0\t\n512\t2\t1000000\t40\t2000000\tYHR174W\n3\t0\t0\t0\t0\t\n"
    );
    let report = directory.analyze_with_args(&data, &["--flowcell-layout", "minion-r9.5"]);
    let view = &report["diagnostics"]["minion_read_visualization"];
    assert_eq!(view["eligible_reads"], 2);
    assert_eq!(view["zero_length_records"], 1);
    assert_eq!(view["missing_events_reads"], 1);
    assert_eq!(view["zero_events_reads"], 1);
    assert!(view["bounds"]["positive_events_per_base"].is_null());
    assert_eq!(view["bounds"]["length_bp"], json!([10, 20]));
    let html = directory.html();
    assert!(html.contains("Positive events/base unavailable"));
    assert!(!html.contains("NaN"));
    assert!(!html.contains("Infinity"));

    // Template events cannot color a 2D read: they describe another signal.
    let two_d = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\tnum_events_template\tsequence_length_2d\tmean_qscore_2d\n1\t0\t100\t20\t1000\t10\t8\n";
    let report = directory.analyze_with_args(two_d, &["--flowcell-layout", "minion-r9.5"]);
    let view = &report["diagnostics"]["minion_read_visualization"];
    assert_eq!(view["eligible_reads"], 1);
    assert_eq!(view["missing_events_reads"], 1);
    assert_eq!(view["points"][0]["length_bp"], 10);
    assert!(view["points"][0]["events_per_base"].is_null());
    for invalid_events in ["-1", "not-a-count", "inf"] {
        // Optional legacy event metadata remains unavailable, never log-mapped.
        let report = directory.analyze_with_args(
            &format!("{header}1\t0\t10\t8\t{invalid_events}\t\n"),
            &["--flowcell-layout", "minion-r9.5"],
        );
        let view = &report["diagnostics"]["minion_read_visualization"];
        assert_eq!(view["missing_events_reads"], 1);
        assert!(view["points"][0]["events_per_base"].is_null());
    }
    for input in [header.to_owned(), format!("{header}1\t0\t0\t0\t0\t\n")] {
        let report = directory.analyze_with_args(&input, &["--flowcell-layout", "minion-r9.5"]);
        assert_eq!(
            report["diagnostics"]["minion_read_visualization"]["eligible_reads"],
            0
        );
        assert!(directory.html().contains("No positive-length observations"));
    }
}

#[test]
#[cfg(feature = "static-figures")]
fn six_static_formats_preserve_metrics_dimensions_names_and_palette() {
    let directory = TestDirectory::new();
    let data = format!("{HEADER}{ROW}");
    let original = directory.analyze(&data);
    let mut stems = None;
    for (format, magic) in [
        ("png", b"\x89PNG\r\n\x1a\n".as_slice()),
        ("pdf", b"%PDF-"),
        ("ps", b"%!PS-Adobe-3.0"),
        ("jpeg", b"\xff\xd8\xff"),
        ("tiff", b"II*\0"),
        ("bmp", b"BM"),
    ] {
        let report = directory
            .analyze_with_args(&data, &["--format", format, "--smallfigures", "--add-stat"]);
        for key in [
            "metrics",
            "quality",
            "diagnostics",
            "comparison",
            "time_bins",
            "channels",
            "distributions",
        ] {
            assert_eq!(report[key], original[key], "{format} changed {key}");
        }
        assert_eq!(report["presentation"]["export_format"], format);
        assert!(directory.html().contains("Mean read Q-score"));
        let folder = directory.0.join("output/figures");
        let names: Vec<String> =
            serde_json::from_slice(&fs::read(folder.join("manifest.json")).unwrap()).unwrap();
        assert!(names.len() > 15);
        let current: Vec<_> = names
            .iter()
            .map(|n| {
                assert!(n.ends_with(&format!(".{format}")));
                let bytes = fs::read(folder.join(n)).unwrap();
                assert!(bytes.starts_with(magic), "bad {format} signature for {n}");
                n.rsplit_once('.').unwrap().0.to_owned()
            })
            .collect();
        assert_eq!(
            current.len(),
            current
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        );
        if let Some(expected) = &stems {
            assert_eq!(&current, expected);
        } else {
            stems = Some(current);
        }
        let bytes = fs::read(folder.join(format!("length-counts.{format}"))).unwrap();
        if matches!(format, "png" | "jpeg" | "tiff" | "bmp") {
            let image = image::load_from_memory(&bytes).unwrap().into_rgb8();
            assert_eq!(image.width(), 1417); // 120 mm at 300 DPI, rounded.
            assert!(
                image.pixels().any(|p| p
                    .0
                    .iter()
                    .zip([69u8, 89, 152])
                    .all(|(&a, b)| a.abs_diff(b) <= 5)),
                "missing canonical indigo palette"
            );
        }
    }
}

#[test]
#[cfg(feature = "static-figures")]
fn failed_static_destination_preserves_reports_figures_and_cleans_staging() {
    let directory = TestDirectory::new();
    directory.analyze(&format!("{HEADER}{ROW}"));
    let folder = directory.0.join("output");
    let old_json = fs::read(folder.join("report.json")).unwrap();
    let old_html = fs::read(folder.join("report.html")).unwrap();
    fs::create_dir(folder.join("figures")).unwrap();
    fs::write(folder.join("figures/cumulative-yield.png"), "previous PNG").unwrap();
    fs::create_dir(folder.join("figures/length-counts.png")).unwrap();
    let input = directory.0.join("input.tsv");
    let output = directory.run_with_args(&input, &["--format", "png"]);
    assert!(!output.status.success());
    assert_eq!(fs::read(folder.join("report.json")).unwrap(), old_json);
    assert_eq!(fs::read(folder.join("report.html")).unwrap(), old_html);
    assert_eq!(
        fs::read_to_string(folder.join("figures/cumulative-yield.png")).unwrap(),
        "previous PNG"
    );
    assert!(
        fs::read_dir(&folder)
            .unwrap()
            .chain(fs::read_dir(folder.join("figures")).unwrap())
            .all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp"))
    );
}

#[test]
#[cfg(not(feature = "static-figures"))]
fn minimal_build_rejects_static_formats_before_analysis() {
    let directory = TestDirectory::new();
    for format in ["png", "pdf", "ps", "jpeg", "tiff", "bmp"] {
        let output = directory.run_with_args(&fixture("synthetic.tsv"), &["--format", format]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("--features static-figures"));
        assert!(!directory.0.join("output").exists());
    }
}

fn report_snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut output = std::collections::BTreeMap::new();
    fn visit(
        root: &Path,
        directory: &Path,
        output: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>,
    ) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, output);
            } else {
                output.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    visit(root, root, &mut output);
    output
}

#[test]
fn parallel_groups_preserve_interleaved_chunks_legacy_empty_and_exact_outputs() {
    let directory = TestDirectory::new();
    let row = |run: &str, length: u64, q: f64| {
        format!("{run}\t1\t2\t900\t1\tTRUE\t{length}\t{q}\tunusual\n")
    };
    // Non-contiguous connected components must not change first-seen run order.
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    let c = directory.0.join("c.tsv");
    let d = directory.0.join("d.tsv");
    let legacy = directory.0.join("legacy.tsv");
    let empty = directory.0.join("empty.tsv");
    fs::write(&a, format!("{HEADER}{}", row("alpha", 10, 7.0))).unwrap();
    fs::write(&b, format!("{HEADER}{}", row("beta", 20, 8.25))).unwrap();
    fs::write(
        &c,
        format!(
            "{HEADER}{}{}{}",
            row("gamma", 40, 11.0),
            row("alpha", 30, 6.75),
            row("gamma", 0, 0.0)
        ),
    )
    .unwrap();
    fs::write(&d, format!("{HEADER}{}", row("beta", 50, 12.0))).unwrap();
    fs::write(
        &legacy,
        "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n2\t0\t60\t9.0\n",
    )
    .unwrap();
    fs::write(&empty, HEADER).unwrap();
    let inputs: [&Path; 6] = [&a, &b, &c, &d, &legacy, &empty];
    let options = [
        "--format",
        "svg",
        "--minionqc-trends",
        "--minionqc-density",
        "--yaml",
    ];
    let report = run_collection(&directory, &inputs, &options);
    assert_eq!(report["metrics"]["total_yield_bp"], 210);
    assert_eq!(report["metrics"]["median_read_length_bp"], 35.0);
    assert_eq!(report["metrics"]["n50_bp"], 50);
    assert_eq!(report["collection"]["runs"][0]["run_id"], "alpha");
    assert_eq!(report["collection"]["runs"][1]["run_id"], "beta");
    assert_eq!(report["collection"]["runs"][2]["run_id"], "gamma");
    let before = report_snapshot(&directory.0.join("output"));
    for workers in ["2", "3", "4"] {
        for strong in [false, true] {
            let mut parallel = options.to_vec();
            parallel.extend(["--processors", workers]);
            if strong {
                parallel.push("--verify-input-content");
            }
            let after = run_collection(&directory, &inputs, &parallel);
            assert_eq!(report, after);
            assert_eq!(before, report_snapshot(&directory.0.join("output")));
        }
    }
    // --group-inputs explicitly joins identity-free chunks, never splits them.
    let grouped = run_collection(&directory, &[&legacy, &empty], &["--group-inputs"]);
    assert_eq!(
        grouped,
        run_collection(
            &directory,
            &[&legacy, &empty],
            &["--group-inputs", "--processors", "2"]
        )
    );
    // A mixed-run single source remains sequential and preserves split semantics.
    let split = run_collection(&directory, &[&c], &["--split-runs", "--combined-only"]);
    assert_eq!(
        split,
        run_collection(
            &directory,
            &[&c],
            &["--split-runs", "--combined-only", "--processors", "2"]
        )
    );
}

#[test]
fn parallel_worker_failure_preserves_previous_collection_and_publishes_nothing_new() {
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    fs::write(&a, format!("{HEADER}{ROW}")).unwrap();
    fs::write(
        &b,
        format!(
            "{HEADER}{}",
            ROW.replace("synthetic-run", "other")
                .replace("\t1\t1\t0", "\t3001\t1\t0")
        ),
    )
    .unwrap();
    run_collection(&directory, &[&a, &b], &[]);
    let before = report_snapshot(&directory.0.join("output"));
    let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .arg("analyze")
        .args([&a, &b])
        .args([
            "--processors",
            "2",
            "--flowcell-layout",
            "promethion-3000",
            "--format",
            "svg",
            "--output",
        ])
        .arg(directory.0.join("output"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("3001"));
    assert_eq!(before, report_snapshot(&directory.0.join("output")));
}

#[test]
fn compatible_logical_flags_preserve_bare_flags_and_explicit_false() {
    let directory = TestDirectory::new();
    let data = format!("{HEADER}{ROW}");
    let on = directory.analyze_with_args(&data, &["--smallfigures", "TRUE", "--add-stat", "TRUE"]);
    assert_eq!(on["presentation"]["small_figures"], true);
    assert_eq!(on["presentation"]["add_stat"], true);
    let off = directory.analyze_with_args(
        &data,
        &[
            "--smallfigures",
            "FALSE",
            "--add-stat=FALSE",
            "--combined-only",
            "FALSE",
        ],
    );
    assert_eq!(off["presentation"]["small_figures"], false);
    assert_eq!(off["presentation"]["add_stat"], false);
    let input = directory.0.join("input.tsv");
    let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .args(["analyze", "--smallfigures"])
        .arg(&input)
        .args(["-o"])
        .arg(directory.0.join("bare"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = directory.run_with_args(&input, &["--combined-only", "TRUE"]);
    assert!(result.status.success());
    assert!(directory.0.join("output/combined/report.json").is_file());
    let result = directory.run_with_args(&input, &["--add-stat=perhaps"]);
    assert!(!result.status.success());
    for n in ["0", "-1", "65536", "many"] {
        assert!(
            !directory
                .run_with_args(&input, &["--processors", n])
                .status
                .success()
        );
    }
}

#[test]
fn compatibility_output_is_opt_in_reserved_and_never_reuses_existing_directory() {
    let directory = TestDirectory::new();
    let input = directory.0.join("reads with spaces.tsv");
    let data = format!("{HEADER}{ROW}");
    fs::write(&input, &data).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
            .arg("analyze")
            .arg(&input)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(!run(&[]).status.success());
    assert!(!directory.0.join("ontrunqc_qc").exists());
    let result = run(&["--minionqc-compat"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let folder = directory.0.join("ontrunqc_qc");
    let snapshot = report_snapshot(&folder);
    assert!(folder.join("report.html").is_file());
    assert!(!folder.join("figures").exists()); // Safe HTML default, even in compatibility mode.
    assert!(!run(&["--minionqc-compat"]).status.success());
    assert_eq!(snapshot, report_snapshot(&folder));
    assert_eq!(data, fs::read_to_string(&input).unwrap());
    let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .arg("analyze")
        .args([&input, &input])
        .arg("--minionqc-compat")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("provide --output"));
    let nested = directory.0.join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("sequencing_summary.tsv"), &data).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .arg("analyze")
        .arg(&nested)
        .args(["--minionqc-compat", "--processors", "2"])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(nested.join("ontrunqc_qc/combined/report.html").is_file());
    let broken = directory.0.join("broken");
    fs::create_dir(&broken).unwrap();
    let bad = broken.join("bad.tsv");
    fs::write(&bad, "not a header").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
        .arg("analyze")
        .arg(&bad)
        .arg("--minionqc-compat")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!broken.join("ontrunqc_qc").exists());
}

#[test]
fn signed_cutoffs_select_legacy_reads_without_weakening_modern_validation() {
    let directory = TestDirectory::new();
    let legacy = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n1\t0\t10\t-2\n1\t0\t20\t-1\n1\t0\t30\t0\n";
    let selected = directory.analyze_with_args(legacy, &["--qscore_cutoff", "-1"]);
    assert_eq!(selected["comparison"]["qscore_selected"]["read_count"], 2);
    assert_eq!(
        selected["comparison"]["qscore_selected"]["total_yield_bp"],
        50
    );
    assert_eq!(selected["metrics"]["basecalled_reads"], 3);
    assert_eq!(
        selected["comparison"]["qscore_selected"]["median_qscore"],
        -0.5
    );
    assert_eq!(selected["comparison"]["qscore_selected"]["n50_bp"], 30);
    assert_eq!(
        selected["comparison"]["quality_histogram"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["read_count"].as_u64().unwrap())
            .sum::<u64>(),
        2
    );
    let a = directory.0.join("negative-a.tsv");
    let b = directory.0.join("negative-b.tsv");
    fs::write(&a, legacy).unwrap();
    fs::write(&b, legacy).unwrap();
    let combined = run_collection(&directory, &[&a, &b], &["--processors", "2", "-q", "-1"]);
    assert_eq!(combined["comparison"]["qscore_selected"]["read_count"], 4);
    assert_eq!(
        combined["comparison"]["quality_histogram"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["read_count"].as_u64().unwrap())
            .sum::<u64>(),
        4
    );
    let modern = format!("{HEADER}{}", ROW.replace("12.5", "-1"));
    let input = directory.0.join("input.tsv");
    fs::write(&input, modern).unwrap();
    let result = directory.run_with_args(&input, &["-q", "-3"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("finite nonnegative"));
    let valid = directory.analyze_with_args(&format!("{HEADER}{ROW}"), &["-q", "-1"]);
    assert_eq!(valid["comparison"]["qscore_selected"]["read_count"], 1);
}

#[test]
fn compatibility_mux_references_follow_integer_hour_source_semantics() {
    let directory = TestDirectory::new();
    let data = format!(
        "{HEADER}{ROW}{}",
        ROW.replace("\t0\t1\tTRUE", "\t61700\t1\tTRUE")
    );
    let normal = directory.analyze(&data);
    assert_eq!(normal["presentation"]["mux_markers_seconds"], json!([]));
    let compatible = directory.analyze_with_args(&data, &["--minionqc-compat"]);
    assert_eq!(
        compatible["presentation"]["mux_markers_seconds"],
        json!([0.0, 61200.0])
    );
    let explicit = directory.analyze_with_args(&data, &["--minionqc-compat", "--muxes", "3.5"]);
    assert_eq!(
        explicit["presentation"]["mux_markers_seconds"],
        json!([0.0, 12600.0, 25200.0, 37800.0, 50400.0])
    );
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv");
    fs::write(&a, &data).unwrap();
    fs::write(&b, data.replace("synthetic-run", "second")).unwrap();
    let combined = run_collection(
        &directory,
        &[&a, &b],
        &["--minionqc-compat", "--muxes", "3.5", "--processors", "2"],
    );
    assert_eq!(
        combined["presentation"]["mux_markers_seconds"],
        json!([0.0, 28800.0, 57600.0])
    );
    let empty = directory.analyze_with_args(HEADER, &["--minionqc-compat"]);
    assert_eq!(empty["presentation"]["mux_markers_seconds"], json!([]));
    let short = directory.analyze_with_args(
        &format!("{HEADER}{}", ROW.replace("\t10\t12.5", "\t0\t0")),
        &["--minionqc-compat"],
    );
    assert_eq!(short["presentation"]["mux_markers_seconds"], json!([0.0]));
    assert_eq!(short["metrics"]["basecalled_reads"], 0);
}

#[test]
fn independent_dense_minion_runs_are_identical_at_one_and_two_workers() {
    let directory = TestDirectory::new();
    let paths = [directory.0.join("a.tsv"), directory.0.join("b.tsv")];
    for (j, path) in paths.iter().enumerate() {
        let mut data = HEADER.to_owned();
        for i in 0..4100 {
            data.push_str(&format!(
                "run-{j}\t{}\t2\t{}\t1\tTRUE\t{}\t{}\tunusual\n",
                i % 512 + 1,
                i * 23,
                i + 10,
                7.5 + (i % 4) as f64
            ));
        }
        fs::write(path, data).unwrap();
    }
    let inputs = [paths[0].as_path(), paths[1].as_path()];
    run_collection(&directory, &inputs, &["--flowcell-layout", "minion-r9.5"]);
    let before = report_snapshot(&directory.0.join("output"));
    for id in ["run_001", "run_002"] {
        let report: Value = serde_json::from_slice(
            &fs::read(directory.0.join(format!("output/runs/{id}/report.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            report["diagnostics"]["minion_read_visualization"]["eligible_reads"],
            4100
        );
    }
    run_collection(
        &directory,
        &inputs,
        &[
            "--flowcell-layout",
            "minion-r9.5",
            "--processors",
            "2",
            "--verify-input-content",
        ],
    );
    assert_eq!(before, report_snapshot(&directory.0.join("output")));
    let compressed = paths[1].with_extension("tsv.gz");
    fs::write(&compressed, gzip(&fs::read(&paths[1]).unwrap())).unwrap();
    for workers in ["1", "2", "4"] {
        run_collection(
            &directory,
            &[&paths[0], &compressed],
            &[
                "--flowcell-layout",
                "minion-r9.5",
                "--processors",
                workers,
                "--verify-input-content",
            ],
        );
        assert_eq!(before, report_snapshot(&directory.0.join("output")));
    }
}

#[test]
fn report_destinations_cannot_overwrite_input_sources() {
    for figure in [false, true] {
        let directory = TestDirectory::new();
        let output = directory.0.join("output");
        fs::create_dir(&output).unwrap();
        fs::create_dir(output.join("figures")).unwrap();
        let source = output.join(if figure {
            "figures/length-counts.svg"
        } else {
            "report.json"
        });
        let data = format!("{HEADER}{ROW}");
        fs::write(&source, &data).unwrap();
        fs::write(output.join("report.html"), "previous valid HTML").unwrap();
        let result =
            directory.run_with_args(&source, &["--format", "svg", "--verify-input-content"]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("overwrite input"));
        assert_eq!(fs::read_to_string(&source).unwrap(), data);
        assert_eq!(
            fs::read_to_string(output.join("report.html")).unwrap(),
            "previous valid HTML"
        );
        assert!(
            report_snapshot(&output)
                .keys()
                .all(|p| p.extension().is_none_or(|e| e != "tmp"))
        );
    }
}

fn gzip(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn gzip_and_plain_inputs_have_identical_reports_across_schemas_and_members() {
    let legacy = "channel\tstart_time\tsequence_length_template\tmean_qscore_template\tcalibration_strand_genome_template\n1\t900\t10\t-1\tNA\n2\t0\t20\t7\tYHR174W\n1\t1800\t0\t-2\tNA\n2\t0\t30\t8\tNA\n";
    let two_d = "channel\tstart_time\tsequence_length_2d\tmean_qscore_2d\tsequence_length_template\tmean_qscore_template\n1\t900\t20\t-1\t100\t9\n2\t0\t40\t8\t200\t10\n";
    let zero = format!("{HEADER}{}", ROW.replace("\t10\t12.5", "\t0\t0"));
    for data in [
        fs::read_to_string(fixture("synthetic.tsv")).unwrap(),
        fs::read_to_string(fixture("reordered.tsv")).unwrap(),
        legacy.into(),
        two_d.into(),
        HEADER.into(),
        zero,
    ] {
        let directory = TestDirectory::new();
        let plain = directory.0.join("input.tsv");
        fs::write(&plain, &data).unwrap();
        let options = [
            "--yaml",
            "--format",
            "svg",
            "--minionqc-trends",
            "--minionqc-density",
            "-q",
            "-1",
        ];
        let result = directory.run_with_args(&plain, &options);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let expected = report_snapshot(&directory.0.join("output"));
        for name in [
            "reads.txt.gz",
            "reads.tsv.gz",
            "arbitrary.gz",
            "no-extension",
        ] {
            let path = directory.0.join(name);
            for multi in [false, true] {
                let bytes = if multi {
                    let middle = data.len() / 2; // Can split a TSV record, not just a line.
                    [
                        gzip(&data.as_bytes()[..middle]),
                        gzip(b""),
                        gzip(&data.as_bytes()[middle..]),
                    ]
                    .concat()
                } else {
                    gzip(data.as_bytes())
                };
                fs::write(&path, bytes).unwrap();
                let mut args = options.to_vec();
                args.push("--verify-input-content");
                let result = directory.run_with_args(&path, &args);
                assert!(
                    result.status.success(),
                    "{name}: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(
                    expected,
                    report_snapshot(&directory.0.join("output")),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn invalid_gzip_and_invalid_decompressed_tsv_never_publish_reports() {
    let data = format!("{HEADER}{ROW}");
    let valid = gzip(data.as_bytes());
    let mut crc = valid.clone();
    let n = crc.len();
    crc[n - 8] ^= 1;
    let mut size = valid.clone();
    size[n - 4] ^= 1;
    let mut signature = valid.clone();
    signature[0] = 0;
    let variants = [
        (Vec::new(), "gzip"),
        (valid[..1].to_vec(), "gzip"),
        (valid[..valid.len() / 2].to_vec(), "gzip"),
        (valid[..valid.len() - 1].to_vec(), "gzip"),
        (crc, "gzip"),
        (size, "gzip"),
        (signature, "gzip"),
        ([valid.clone(), b"garbage".to_vec()].concat(), "gzip"),
        ([valid.clone(), vec![0; 16]].concat(), "gzip"),
        (
            [valid.clone(), gzip(b"later member")[..12].to_vec()].concat(),
            "gzip",
        ),
        (data.as_bytes().to_vec(), "gzip"),
        (gzip(b""), "empty input"),
        (
            gzip(format!("{HEADER}{}", ROW.replace("12.5", "bad")).as_bytes()),
            "line 2",
        ),
    ];
    for (bytes, message) in variants {
        let directory = TestDirectory::new();
        let input = directory.0.join("input.tsv.gz");
        fs::write(&input, &valid).unwrap();
        assert!(directory.run(&input).status.success());
        let expected = report_snapshot(&directory.0.join("output"));
        fs::write(&input, bytes).unwrap();
        let result = directory.run_with_args(&input, &["--verify-input-content"]);
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(
            error.contains(message) && error.contains("input.tsv.gz"),
            "{error}"
        );
        if message == "gzip" {
            assert!(error.contains("aggregation pass"), "{error}");
        }
        assert_eq!(expected, report_snapshot(&directory.0.join("output")));
    }
}

#[test]
fn mixed_gzip_chunks_discovery_and_parallel_groups_preserve_every_output() {
    let directory = TestDirectory::new();
    let inputs = directory.0.join("inputs with spaces");
    fs::create_dir(&inputs).unwrap();
    let chunks = [
        format!("{HEADER}{}", ROW.replace("synthetic-run", "alpha")),
        format!("{HEADER}{}", ROW.replace("synthetic-run", "beta")),
        format!(
            "{HEADER}{}{}",
            ROW.replace("synthetic-run", "alpha"),
            ROW.replace("synthetic-run", "gamma")
        ),
        "channel\tstart_time\tsequence_length_2d\tmean_qscore_2d\n1\t0\t25\t-1\n".into(),
        HEADER.into(),
    ];
    let paths: Vec<_> = (0..chunks.len())
        .map(|i| inputs.join(format!("sequencing_summary_{i}.tsv")))
        .collect();
    for (path, data) in paths.iter().zip(&chunks) {
        fs::write(path, data).unwrap();
    }
    // Unrelated files must stay excluded, even when compressed.
    fs::write(inputs.join("unrelated.txt.gz"), b"bad gzip").unwrap();
    let options = [
        "--yaml",
        "--format",
        "svg",
        "--minionqc-trends",
        "--minionqc-density",
    ];
    run_collection(&directory, &[&inputs], &options);
    let expected = report_snapshot(&directory.0.join("output"));
    for i in [0, 2, 3, 4] {
        let path = paths[i].with_extension("tsv.gz");
        fs::write(&path, gzip(chunks[i].as_bytes())).unwrap();
        fs::remove_file(&paths[i]).unwrap();
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&inputs, inputs.join("directory-cycle")).unwrap();
        fs::hard_link(
            inputs.join("sequencing_summary_0.tsv.gz"),
            inputs.join("sequencing_summary_alias.txt.gz"),
        )
        .unwrap();
    }
    for workers in ["1", "2", "4"] {
        let mut args = options.to_vec();
        args.extend(["--processors", workers, "--verify-input-content"]);
        run_collection(&directory, &[&inputs], &args);
        assert_eq!(expected, report_snapshot(&directory.0.join("output")));
    }
    let mixed = inputs.join("sequencing_summary_2.tsv.gz");
    run_collection(
        &directory,
        &[&mixed],
        &[
            "--split-runs",
            "--combined-only",
            "--processors",
            "4",
            "--verify-input-content",
        ],
    );
    let split: Value =
        serde_json::from_slice(&fs::read(directory.0.join("output/combined/report.json")).unwrap())
            .unwrap();
    assert_eq!(split["metrics"]["total_records"], 2);
    assert_eq!(split["collection"]["runs"].as_array().unwrap().len(), 2);
}

#[test]
fn gzip_identity_free_grouping_and_dense_minion_passes_are_compression_independent() {
    let directory = TestDirectory::new();
    let legacy =
        "channel\tstart_time\tsequence_length_template\tmean_qscore_template\n1\t0\t20\t-1\n";
    let a = directory.0.join("a.tsv");
    let b = directory.0.join("b.tsv.gz");
    fs::write(&a, legacy).unwrap();
    fs::write(&b, gzip(legacy.as_bytes())).unwrap();
    let report = run_collection(
        &directory,
        &[&a, &b],
        &[
            "--group-inputs",
            "--processors",
            "4",
            "--verify-input-content",
        ],
    );
    assert_eq!(report["collection"]["runs"].as_array().unwrap().len(), 1);
    assert_eq!(report["metrics"]["total_yield_bp"], 40);
    let data: String = (0..4100)
        .map(|i| {
            format!(
                "synthetic-run\t{}\t1\t{}\t1\tTRUE\t{}\t{}\tunknown\n",
                i % 512 + 1,
                i * 23,
                i + 1,
                7 + i % 10
            )
        })
        .collect();
    fs::write(&a, format!("{HEADER}{data}")).unwrap();
    let options = [
        "--flowcell-layout",
        "minion-r9.5",
        "--format",
        "svg",
        "--yaml",
        "--minionqc-trends",
        "--minionqc-density",
    ];
    assert!(directory.run_with_args(&a, &options).status.success());
    let expected = report_snapshot(&directory.0.join("output"));
    fs::write(&b, gzip(format!("{HEADER}{data}").as_bytes())).unwrap();
    let mut args = options.to_vec();
    args.push("--verify-input-content");
    let result = directory.run_with_args(&b, &args);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(expected, report_snapshot(&directory.0.join("output")));
    let report: Value =
        serde_json::from_slice(&fs::read(directory.0.join("output/report.json")).unwrap()).unwrap();
    assert_eq!(
        report["diagnostics"]["minion_read_visualization"]["eligible_reads"],
        4100
    );
    assert_eq!(
        report["diagnostics"]["minion_read_visualization"]["additional_input_passes"],
        1
    );
}

#[test]
#[cfg(feature = "static-figures")]
fn gzip_native_png_exports_are_byte_identical() {
    let directory = TestDirectory::new();
    let data = fs::read(fixture("synthetic.tsv")).unwrap();
    let a = directory.0.join("input.tsv");
    let b = directory.0.join("input.tsv.gz");
    fs::write(&a, &data).unwrap();
    fs::write(&b, gzip(&data)).unwrap();
    let options = ["--format", "png", "--smallfigures", "--add-stat", "--yaml"];
    assert!(directory.run_with_args(&a, &options).status.success());
    let expected = report_snapshot(&directory.0.join("output"));
    assert!(directory.run_with_args(&b, &options).status.success());
    assert_eq!(expected, report_snapshot(&directory.0.join("output")));
}

#[test]
fn gzip_python_encoded_fixture_and_parallel_discovery_failure() {
    use base64::Engine;
    // Python 3 gzip.compress(payload, mtime=0), backed by zlib rather than miniz.
    let encoded = "H4sIAAAAAAAA/z3IMQ6AMAgF0Jmept28DSHkx5oA2oL3d/ONT6dEwChLdnFdDkqsF6FgQ5w1ueCPSYEcErxS740/26BOo9PRPpZ5GKVKAAAA";
    let directory = TestDirectory::new();
    let a = directory.0.join("a.tsv.gz");
    let b = directory.0.join("b.tsv.gz");
    let data = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    fs::write(&a, &data).unwrap();
    let report = run_collection(
        &directory,
        &[&a],
        &["--verify-input-content", "--split-runs"],
    );
    assert_eq!(report["metrics"]["total_yield_bp"], 10);
    let expected = report_snapshot(&directory.0.join("output"));
    fs::write(&b, [&data[..], b"invalid trailing member"].concat()).unwrap();
    for workers in ["2", "4"] {
        let result = Command::new(env!("CARGO_BIN_EXE_ontrunqc"))
            .arg("analyze")
            .args([&a, &b])
            .args([
                "--processors",
                workers,
                "--verify-input-content",
                "--output",
            ])
            .arg(directory.0.join("output"))
            .output()
            .unwrap();
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(
            error.contains("b.tsv.gz")
                && error.contains("gzip")
                && error.contains("independent-component discovery"),
            "{error}"
        );
        assert_eq!(expected, report_snapshot(&directory.0.join("output")));
    }
}

#[test]
fn gzip_suffix_does_not_change_source_order_for_prefix_colliding_names() {
    let directory = TestDirectory::new();
    let sources = directory.0.join("sources");
    fs::create_dir(&sources).unwrap();
    let first = sources.join("sequencing_summary.tsv");
    let second = sources.join("sequencing_summary.tsv.extra.tsv");
    fs::write(
        &first,
        format!("{HEADER}{}", ROW.replace("synthetic-run", "alpha")),
    )
    .unwrap();
    fs::write(
        &second,
        format!("{HEADER}{}", ROW.replace("synthetic-run", "beta")),
    )
    .unwrap();
    let report = run_collection(&directory, &[&sources], &["--yaml", "--processors", "2"]);
    assert_eq!(report["collection"]["runs"][0]["run_id"], "alpha");
    let expected = report_snapshot(&directory.0.join("output"));
    fs::write(
        first.with_extension("tsv.gz"),
        gzip(&fs::read(&first).unwrap()),
    )
    .unwrap();
    fs::remove_file(first).unwrap();
    run_collection(
        &directory,
        &[&sources],
        &["--yaml", "--processors", "4", "--verify-input-content"],
    );
    assert_eq!(expected, report_snapshot(&directory.0.join("output")));
}
