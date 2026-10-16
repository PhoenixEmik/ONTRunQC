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
    assert!(cell.contains("preserveAspectRatio=\"none\" aria-hidden=\"true\""));
    assert!(cell.contains("<rect class=\"bar-track\" width=\"100\" height=\"4\"/>"));
    assert!(cell.contains("<rect class=\"bar-fill\" width=\"100.00\" height=\"4\"/>"));
    assert!(cell.ends_with("</svg>"));
    assert!(table.contains("</svg></td><td>1</td><td>10</td>"));
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
    assert_eq!(html.matches("<tr data-channel=").count(), 4096);
    assert_eq!(html.matches("class=\"channel-point\"").count(), 4096);
    assert!(html.contains("data-channel=\"4096\""));
    assert!(html.contains("id=\"channel-pagination\""));
    assert!(html.contains("id=\"channel-search\""));
    assert!(
        html.len() < 4_000_000,
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
