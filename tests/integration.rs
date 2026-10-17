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
            "poregauge-test-{}-{}",
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
        Command::new(env!("CARGO_BIN_EXE_poregauge"))
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
        6000
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
    assert_eq!(html.matches("class=\"heatmap-cell\"").count(), 6000);
    assert!(
        // Ten added scientific comparison charts; never one SVG per table row.
        html.matches("<svg ").count() <= 40,
        "SVG count must follow scientific charts, not table rows"
    );
    assert!(
        html.len() < 9_000_000,
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
    for q in ["NaN", "inf", "-1", "text"] {
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
