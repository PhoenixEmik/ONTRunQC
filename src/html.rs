//! Offline presentation of the final report; no read records or QC aggregation.
use std::fmt::Write;

use crate::report::Report;

const CSS: &str = include_str!("report.css");
const SCRIPT: &str = include_str!("report.js");
const LEFT: f64 = 80.0;
const RIGHT: f64 = 714.0;
const TOP: f64 = 38.0;
const BOTTOM: f64 = 234.0;

pub fn render(report: &Report) -> String {
    let mut out = String::with_capacity(32_768);
    out.push_str(r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'; form-action 'none'">
<title>PoreGauge · Sequencing QC report</title><style>"##);
    out.push_str(CSS);
    out.push_str(r##"</style></head><body>
<a class="skip-link" href="#overview">Skip to report</a>
<header class="masthead"><div class="brand"><svg viewBox="0 0 32 32" aria-hidden="true"><path d="M3 21h5V11h5v15h6V6h5v15h5"/></svg><h1>PoreGauge</h1></div><span class="edition">SEQUENCING QUALITY CONTROL</span></header>
<div class="layout"><nav aria-label="Report sections"><span class="nav-label">IN THIS REPORT</span>
<a href="#overview">01 <span>Run overview</span></a><a href="#yield">02 <span>Sequencing yield</span></a>
<a href="#length">03 <span>Read length</span></a><a href="#quality">04 <span>Read quality</span></a>
<a href="#channels">05 <span>Channels</span></a><a href="#mux">06 <span>Mux performance</span></a>
<a href="#termination">07 <span>Termination reasons</span></a><a href="#speed">08 <span>Sequencing speed</span></a>
<div class="nav-note">One run.<br>Summary-derived QC.<br>Available offline.</div></nav><main>"##);
    overview(&mut out, report);
    sequencing_yield(&mut out, report);
    lengths(&mut out, report);
    quality(&mut out, report);
    channels(&mut out, report);
    muxes(&mut out, report);
    termination(&mut out, report);
    sequencing_speed(&mut out, report);
    write!(out, "<footer>PoreGauge {} · JSON schema {} · Post-sequencing QC from sequencing summaries. No automatic QC judgment.</footer></main></div><script>",
        escape(report.tool.version), report.schema_version).unwrap();
    out.push_str(SCRIPT);
    out.push_str("</script></body></html>\n");
    out
}

fn section(out: &mut String, id: &str, number: &str, title: &str, note: &str) {
    write!(out, "<section id=\"{id}\" aria-labelledby=\"{id}-heading\"><div class=\"section-heading\"><span class=\"section-number\">{number}</span><div><h2 id=\"{id}-heading\">{title}</h2><p>{note}</p></div></div>").unwrap();
}

fn overview(out: &mut String, report: &Report) {
    let m = &report.metrics;
    section(
        out,
        "overview",
        "01",
        "Run overview",
        "Oxford Nanopore sequencing summary · Single-run analysis",
    );
    let run_id = report.run.run_id.as_deref().unwrap_or("Not available");
    write!(
        out,
        "<dl class=\"overview\"><div class=\"run-identity\"><dt>Run ID</dt><dd>{}</dd></div>",
        escape(run_id)
    )
    .unwrap();
    metric(
        out,
        "total_records",
        "Summary records",
        Some(m.total_records.to_string()),
        count(m.total_records),
    );
    metric(
        out,
        "basecalled_reads",
        "Basecalled reads",
        Some(m.basecalled_reads.to_string()),
        count(m.basecalled_reads),
    );
    metric(
        out,
        "total_yield_bp",
        "Total yield",
        Some(m.total_yield_bp.to_string()),
        format!("{} Gb", decimal(m.total_yield_bp as f64 / 1e9)),
    );
    metric(
        out,
        "passed_yield_bp",
        "Passed yield",
        Some(m.passed_yield_bp.to_string()),
        format!("{} Gb", decimal(m.passed_yield_bp as f64 / 1e9)),
    );
    let rate = m.pass_rate_percent();
    metric(
        out,
        "pass_rate_percent",
        "Pass rate · reads",
        rate.map(|v| v.to_string()),
        optional(rate, 1.0, "%"),
    );
    metric(
        out,
        "n50_bp",
        "Read N50",
        m.n50_bp.map(|v| v.to_string()),
        optional(m.n50_bp.map(|v| v as f64), 1e3, "kb"),
    );
    metric(
        out,
        "mean_read_length_bp",
        "Mean read length",
        m.mean_read_length_bp.map(|v| v.to_string()),
        optional(m.mean_read_length_bp, 1e3, "kb"),
    );
    metric(
        out,
        "mean_qscore",
        "Mean read Q-score",
        report.quality.mean_qscore.map(|v| v.to_string()),
        optional(report.quality.mean_qscore, 1.0, ""),
    );
    out.push_str("</dl>");
    write!(out, "<p class=\"annotation\"><strong>{}</strong> zero-length records excluded from basecalled-read statistics. Pass rate = native <code>passes_filtering=TRUE</code> reads / basecalled reads; no additional Q-score threshold.</p>", count(m.zero_length_records)).unwrap();
    if m.basecalled_reads == 0 {
        out.push_str("<p class=\"notice\">No basecalled reads are available. Record counts remain valid; length and quality statistics are not available.</p>");
    }
    out.push_str("</section>");
}

fn metric(out: &mut String, key: &str, label: &str, raw: Option<String>, display: String) {
    write!(out, "<div class=\"metric\"><dt>{label}</dt><dd data-metric=\"{key}\" data-value=\"{}\" class=\"{}\">{}</dd></div>",
        escape(raw.as_deref().unwrap_or("")), if raw.is_none() { "unavailable" } else { "" }, escape(&display)).unwrap();
}

#[derive(Clone, Copy)]
enum TimePlot {
    Cumulative,
    Yield,
    Reads,
}

fn sequencing_yield(out: &mut String, report: &Report) {
    section(
        out,
        "yield",
        "02",
        "Sequencing yield",
        "Time is relative to sequencing run start. Each interval is 15 minutes.",
    );
    out.push_str("<p class=\"annotation\">All bases are attributed to read <strong>start time</strong>. These charts do not reconstruct exact instrument throughput. Cumulative yield steps at interval ends; missing intervals contribute no additional yield.</p>");
    time_plot(
        out,
        report,
        "cumulative-yield",
        "Cumulative yield",
        TimePlot::Cumulative,
    );
    out.push_str("<div class=\"chart-grid\">");
    time_plot(
        out,
        report,
        "interval-yield",
        "Yield per 15-minute interval",
        TimePlot::Yield,
    );
    time_plot(
        out,
        report,
        "interval-reads",
        "Basecalled reads per interval",
        TimePlot::Reads,
    );
    out.push_str("</div>");
    diagnostic_time_plot(out, report, false);
    out.push_str("<p class=\"annotation\">Observed basecalled-producing channels are counted once per 15-minute read-start interval. This is not MinKNOW active pore count, pore occupancy, or pore availability. Missing intervals contain no records; line segments do not bridge missing intervals.</p><details><summary>View exact interval values</summary><div class=\"table-scroll\"><table><caption>Occupied time bins; omitted intervals contain no records.</caption><thead><tr><th class=\"label\" scope=\"col\">Start–end (s)</th><th scope=\"col\">Records</th><th scope=\"col\">Basecalled reads</th><th scope=\"col\">Yield (bp)</th><th scope=\"col\">Cumulative (bp)</th><th scope=\"col\">Producing channels</th></tr></thead><tbody>");
    for (bin, activity) in report
        .time_bins
        .iter()
        .zip(&report.diagnostics.channel_activity)
    {
        write!(
            out,
            "<tr><th class=\"label\" scope=\"row\">{}–{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            count(bin.start_seconds),
            count(bin.end_seconds),
            count(bin.total_records),
            count(bin.basecalled_reads),
            count(bin.yield_bp),
            count(bin.cumulative_yield_bp),
            count(activity.basecalled_producing_channels)
        )
        .unwrap();
    }
    if report.time_bins.is_empty() {
        empty_row(out, 6, "No time-bin observations.");
    }
    out.push_str("</tbody></table></div></details></section>");
}

fn time_plot(out: &mut String, report: &Report, id: &str, title: &str, kind: TimePlot) {
    figure(out, id, title);
    let value = |bin: &crate::metrics::TimeBin| match kind {
        TimePlot::Cumulative => bin.cumulative_yield_bp,
        TimePlot::Yield => bin.yield_bp,
        TimePlot::Reads => bin.basecalled_reads,
    };
    let max = report.time_bins.iter().map(&value).max().unwrap_or(0);
    if max == 0 {
        empty_chart(
            out,
            if report.time_bins.is_empty() {
                "No time-bin observations."
            } else {
                "No basecalled observations in these intervals."
            },
        );
        return;
    }
    let (scale, unit) = match kind {
        TimePlot::Reads => (1.0, "reads"),
        _ => bp_scale(max),
    };
    let (ymax, ticks) = plot_scale(max as f64 / scale, scale == 1.0);
    let end = report.time_bins.last().unwrap().end_seconds as f64;
    let x = |seconds: u64| LEFT + seconds as f64 / end * (RIGHT - LEFT);
    let y = |v: u64| BOTTOM - v as f64 / scale / ymax * (BOTTOM - TOP);
    svg_start(out, id, title, 294);
    axes(
        out,
        ymax,
        ticks,
        &format!(
            "{} ({unit})",
            if matches!(kind, TimePlot::Reads) {
                "Basecalled reads"
            } else {
                "Yield"
            }
        ),
    );
    for i in 0..=4 {
        x_tick(
            out,
            LEFT + (RIGHT - LEFT) * i as f64 / 4.0,
            &decimal(end / 3600.0 * i as f64 / 4.0),
        );
    }
    out.push_str("<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Time from run start (h)</text>");
    if matches!(kind, TimePlot::Cumulative) {
        let mut path = format!("M {LEFT} {BOTTOM}");
        let mut previous = 0;
        for bin in &report.time_bins {
            write!(
                path,
                " L {:.2} {:.2} L {:.2} {:.2}",
                x(bin.end_seconds),
                y(previous),
                x(bin.end_seconds),
                y(bin.cumulative_yield_bp)
            )
            .unwrap();
            previous = bin.cumulative_yield_bp;
        }
        write!(out, "<path class=\"cumulative-line\" d=\"{path}\"/>").unwrap();
    }
    for bin in &report.time_bins {
        let v = value(bin);
        let label = if matches!(kind, TimePlot::Cumulative) {
            format!(
                "Through {} seconds: {} bp cumulative",
                bin.end_seconds,
                count(v)
            )
        } else {
            format!(
                "[{}, {}) seconds: {} {}",
                bin.start_seconds,
                bin.end_seconds,
                count(v),
                if matches!(kind, TimePlot::Reads) {
                    "reads"
                } else {
                    "bp"
                }
            )
        };
        if matches!(kind, TimePlot::Cumulative) {
            write!(out, "<circle class=\"primary\" cx=\"{:.2}\" cy=\"{:.2}\" r=\"2.5\"><title>{}</title></circle>", x(bin.end_seconds), y(v), escape(&label)).unwrap();
        } else {
            let width = x(bin.end_seconds) - x(bin.start_seconds);
            write!(out, "<rect class=\"primary\" x=\"{:.2}\" y=\"{:.2}\" width=\"{:.3}\" height=\"{:.2}\"><title>{}</title></rect>",
                x(bin.start_seconds), y(v), width * 0.9, BOTTOM - y(v), escape(&label)).unwrap();
        }
    }
    out.push_str("</svg></div></figure>");
}

fn lengths(out: &mut String, report: &Report) {
    section(
        out,
        "length",
        "03",
        "Read length distribution",
        "Exact run statistics; distribution plots use the recorded histogram intervals.",
    );
    write!(out, "<div class=\"stat-line\"><span>N50 <strong>{}</strong></span><span>Median length <strong data-metric=\"median_read_length_bp\" data-value=\"{}\">{}</strong></span></div>",
        optional(report.metrics.n50_bp.map(|v| v as f64), 1e3, "kb"), report.metrics.median_read_length_bp.map(|v| v.to_string()).unwrap_or_default(), optional(report.metrics.median_read_length_bp, 1e3, "kb")).unwrap();
    read_comparison(out, report);
    out.push_str("<p class=\"annotation\">Intervals are lower-inclusive and upper-exclusive, in bp. The final interval is open-ended. These are categorical bins; bar spacing does not imply equal length ranges. Zero-length records are excluded.</p><div class=\"chart-grid\">");
    let bins = &report.read_length_distribution;
    let reads: Vec<_> = bins
        .iter()
        .map(|b| {
            (
                length_interval(b.lower_inclusive_bp, b.upper_exclusive_bp),
                b.read_count,
                "primary",
            )
        })
        .collect();
    bars(
        out,
        "length-counts",
        "Reads by length interval",
        "Basecalled reads (reads)",
        1.0,
        &reads,
    );
    let yields: Vec<_> = bins
        .iter()
        .map(|b| {
            (
                length_interval(b.lower_inclusive_bp, b.upper_exclusive_bp),
                b.yield_bp,
                "primary",
            )
        })
        .collect();
    let (scale, unit) = bp_scale(bins.iter().map(|b| b.yield_bp).max().unwrap_or(0));
    bars(
        out,
        "length-yield",
        "Yield by length interval",
        &format!("Yield ({unit})"),
        scale,
        &yields,
    );
    out.push_str("</div><details><summary>View exact length histogram</summary><div class=\"table-scroll\"><table><caption>Basecalled read length intervals</caption><thead><tr><th class=\"label\" scope=\"col\">Length interval (bp)</th><th scope=\"col\">Reads</th><th scope=\"col\">Yield (bp)</th></tr></thead><tbody>");
    for b in bins {
        write!(
            out,
            "<tr><th class=\"label\" scope=\"row\">{}</th><td>{}</td><td>{}</td></tr>",
            length_interval(b.lower_inclusive_bp, b.upper_exclusive_bp),
            count(b.read_count),
            count(b.yield_bp)
        )
        .unwrap();
    }
    if bins.is_empty() {
        empty_row(out, 3, "No histogram intervals.");
    }
    out.push_str("</tbody></table></div></details>");
    read_time_plot(out, report, false);
    out.push_str("</section>");
}

fn quality(out: &mut String, report: &Report) {
    section(
        out,
        "quality",
        "04",
        "Read quality",
        "Per-read Q-scores and native filtering outcomes.",
    );
    write!(out, "<div class=\"stat-line\"><span>Mean Q-score <strong>{}</strong></span><span>Median Q-score <strong data-metric=\"median_qscore\" data-value=\"{}\">{}</strong></span></div>",
        optional(report.quality.mean_qscore, 1.0, ""), report.quality.median_qscore.map(|v| v.to_string()).unwrap_or_default(), optional(report.quality.median_qscore, 1.0, "")).unwrap();
    out.push_str("<p class=\"annotation\">The mean is the arithmetic mean of per-read Q-scores, not an aggregate base error probability. Zero-length records are excluded. Histogram boundaries do not impose filtering thresholds.</p>");
    let histogram: Vec<_> = report
        .quality
        .histogram
        .iter()
        .map(|b| {
            (
                quality_interval(b.lower_inclusive, b.upper_exclusive),
                b.read_count,
                "primary",
            )
        })
        .collect();
    bars(
        out,
        "quality-histogram",
        "Read Q-score distribution",
        "Basecalled reads (reads)",
        1.0,
        &histogram,
    );
    out.push_str("<div class=\"chart-grid\">");
    let m = &report.metrics;
    bars(
        out,
        "filter-reads",
        "Pass / fail reads",
        "Basecalled reads (reads)",
        1.0,
        &[
            ("Pass".into(), m.passed_reads, "passed"),
            ("Fail".into(), m.failed_reads, "failed"),
        ],
    );
    let (scale, unit) = bp_scale(m.passed_yield_bp.max(m.failed_yield_bp));
    bars(
        out,
        "filter-yield",
        "Pass / fail yield",
        &format!("Yield ({unit})"),
        scale,
        &[
            ("Pass".into(), m.passed_yield_bp, "passed"),
            ("Fail".into(), m.failed_yield_bp, "failed"),
        ],
    );
    out.push_str("</div><p class=\"annotation\"><span class=\"legend pass\">Pass</span><span class=\"legend fail\">Fail</span> Native <code>passes_filtering</code> categories; no automatic run-level QC judgment.</p><details><summary>View exact Q-score histogram</summary><div class=\"table-scroll\"><table><caption>Per-read Q-score intervals, lower-inclusive and upper-exclusive</caption><thead><tr><th class=\"label\" scope=\"col\">Q-score interval</th><th scope=\"col\">Reads</th></tr></thead><tbody>");
    for b in &report.quality.histogram {
        write!(
            out,
            "<tr><th class=\"label\" scope=\"row\">{}</th><td>{}</td></tr>",
            quality_interval(b.lower_inclusive, b.upper_exclusive),
            count(b.read_count)
        )
        .unwrap();
    }
    if report.quality.histogram.is_empty() {
        empty_row(out, 2, "No histogram intervals.");
    }
    out.push_str("</tbody></table></div></details>");
    read_time_plot(out, report, true);
    length_quality_heatmaps(out, report);
    out.push_str("</section>");
}

fn read_comparison(out: &mut String, report: &Report) {
    let all = &report.metrics;
    let passed = &report.diagnostics.passed_read_statistics;
    out.push_str("<div class=\"table-scroll comparison-table\"><table id=\"read-statistics\"><caption>All basecalled reads versus native passed reads · zero-length records excluded</caption><thead><tr><th class=\"label\" scope=\"col\">Metric</th><th scope=\"col\">All basecalled reads</th><th class=\"passed-heading\" scope=\"col\">Passed reads</th></tr></thead><tbody>");
    for (label, key, a, p) in [
        (
            "Read count",
            "read_count",
            Some(all.basecalled_reads),
            Some(passed.read_count),
        ),
        (
            "Total yield (bp)",
            "total_yield_bp",
            Some(all.total_yield_bp),
            Some(passed.total_yield_bp),
        ),
        ("N50 (bp)", "n50_bp", all.n50_bp, passed.n50_bp),
        (
            "Minimum length (bp)",
            "min_read_length_bp",
            all.min_read_length_bp,
            passed.min_read_length_bp,
        ),
        (
            "Maximum length (bp)",
            "max_read_length_bp",
            all.max_read_length_bp,
            passed.max_read_length_bp,
        ),
    ] {
        comparison_row(
            out,
            label,
            key,
            (
                a.map(|v| v.to_string()),
                a.map(count).unwrap_or_else(|| "Not available".into()),
            ),
            (
                p.map(|v| v.to_string()),
                p.map(count).unwrap_or_else(|| "Not available".into()),
            ),
        );
    }
    for (label, key, a, p) in [
        (
            "Mean length (bp)",
            "mean_read_length_bp",
            all.mean_read_length_bp,
            passed.mean_read_length_bp,
        ),
        (
            "Median length (bp)",
            "median_read_length_bp",
            all.median_read_length_bp,
            passed.median_read_length_bp,
        ),
        (
            "Mean Q-score",
            "mean_qscore",
            report.quality.mean_qscore,
            passed.mean_qscore,
        ),
        (
            "Median Q-score",
            "median_qscore",
            report.quality.median_qscore,
            passed.median_qscore,
        ),
    ] {
        comparison_row(
            out,
            label,
            key,
            (a.map(|v| v.to_string()), optional(a, 1.0, "")),
            (p.map(|v| v.to_string()), optional(p, 1.0, "")),
        );
    }
    out.push_str("</tbody></table></div><p class=\"annotation\">Both subsets use exact sorted middle values and N50, not histogram estimates. Passed reads use only <code>passes_filtering=TRUE</code>, with no Q-score cutoff. Mean Q-score is arithmetic per read.</p>");
}

fn comparison_row(
    out: &mut String,
    label: &str,
    key: &str,
    all: (Option<String>, String),
    passed: (Option<String>, String),
) {
    write!(
        out,
        "<tr><th class=\"label\" scope=\"row\">{}</th>",
        escape(label)
    )
    .unwrap();
    for (subset, (raw, display)) in [("all", all), ("passed", passed)] {
        write!(
            out,
            "<td data-metric=\"{subset}_{key}\" data-value=\"{}\">{}</td>",
            escape(raw.as_deref().unwrap_or("")),
            escape(&display)
        )
        .unwrap();
    }
    out.push_str("</tr>");
}

fn read_time_plot(out: &mut String, report: &Report, quality: bool) {
    let (id, title, axis) = if quality {
        (
            "qscore-time",
            "Mean read Q-score over time",
            "Mean read Q-score",
        )
    } else {
        (
            "length-time",
            "Mean read length over time",
            "Mean read length (kb)",
        )
    };
    let mut series = [
        TimeSeries {
            label: "All basecalled reads",
            color: "primary",
            points: Vec::new(),
        },
        TimeSeries {
            label: "Passed reads",
            color: "passed",
            points: Vec::new(),
        },
    ];
    for bin in &report.time_bins {
        let (all, passed) = if quality {
            (bin.mean_qscore, bin.mean_passed_qscore)
        } else {
            (
                bin.mean_read_length_bp.map(|v| v / 1000.0),
                bin.mean_passed_read_length_bp.map(|v| v / 1000.0),
            )
        };
        series[0].points.push((bin.start_seconds, all));
        series[1].points.push((bin.start_seconds, passed));
    }
    time_series_plot(out, report, id, title, axis, &series, false);
    out.push_str("<p class=\"annotation\">Arithmetic means in 15-minute read-start bins; no smoothing. Zero-length records are excluded. Missing or unavailable intervals break each line independently. These observations do not reconstruct read completion time.</p><details><summary>View interval means and denominators</summary><div class=\"table-scroll\"><table><caption>Read-start intervals; length means in bp, Q-score means per read. Unrounded means are retained in data-value attributes and JSON.</caption><thead><tr><th class=\"label\" scope=\"col\">Start–end (s)</th><th scope=\"col\">Basecalled reads</th><th scope=\"col\">Passed reads</th><th scope=\"col\">All mean</th><th scope=\"col\">Passed mean</th></tr></thead><tbody>");
    for bin in &report.time_bins {
        let (all, passed) = if quality {
            (bin.mean_qscore, bin.mean_passed_qscore)
        } else {
            (bin.mean_read_length_bp, bin.mean_passed_read_length_bp)
        };
        write!(out, "<tr><th class=\"label\" scope=\"row\">{}–{}</th><td>{}</td><td>{}</td><td data-value=\"{}\">{}</td><td data-value=\"{}\">{}</td></tr>", count(bin.start_seconds), count(bin.end_seconds), count(bin.basecalled_reads), count(bin.passed_reads), all.map(|v| v.to_string()).unwrap_or_default(), optional(all, 1.0, ""), passed.map(|v| v.to_string()).unwrap_or_default(), optional(passed, 1.0, "")).unwrap();
    }
    if report.time_bins.is_empty() {
        empty_row(out, 5, "No time-bin observations.");
    }
    out.push_str("</tbody></table></div></details>");
}

fn length_quality_heatmaps(out: &mut String, report: &Report) {
    let h = &report.diagnostics.length_quality_histogram;
    out.push_str("<h3 id=\"length-quality\">Read length vs Q-score</h3><p class=\"annotation\">Log₂ read-length intervals and 2-point Q-score intervals; zero-length records excluded. The final length column (≥ 1,048,576 bp) and Q-score row (≥ 50) are open-ended overflow bins, drawn with dashed borders. High values remain counted. Both maps use the same indigo log-count scale; filtering is the native flag.</p><div class=\"chart-grid\">");
    let maximum = h
        .all_read_counts
        .iter()
        .flatten()
        .copied()
        .max()
        .unwrap_or(0);
    for (id, title, counts) in [
        (
            "length-quality-all",
            "Read length vs Q-score · All basecalled reads",
            &h.all_read_counts,
        ),
        (
            "length-quality-passed",
            "Read length vs Q-score · Passed reads",
            &h.passed_read_counts,
        ),
    ] {
        length_quality_map(out, h, id, title, counts, maximum);
    }
    out.push_str("</div><details><summary>View exact length × Q-score cell counts</summary><div class=\"table-scroll\"><table><caption>Nonzero all-read cells; omitted cells contain zero reads in both subsets. Intervals are lower-inclusive and upper-exclusive; final intervals are unbounded.</caption><thead><tr><th class=\"label\" scope=\"col\">Length interval (bp)</th><th class=\"label\" scope=\"col\">Q-score interval</th><th scope=\"col\">All reads</th><th scope=\"col\">Passed reads</th></tr></thead><tbody>");
    let mut populated = false;
    for (q, row) in h.all_read_counts.iter().enumerate() {
        for (l, &reads) in row.iter().enumerate() {
            if reads == 0 {
                continue;
            }
            populated = true;
            write!(out, "<tr><th class=\"label\" scope=\"row\">{}</th><td class=\"label\">{}</td><td>{}</td><td>{}</td></tr>", length_interval(h.length_lower_bounds_bp[l], h.length_lower_bounds_bp.get(l+1).copied()), quality_interval(h.qscore_lower_bounds[q], h.qscore_lower_bounds.get(q+1).copied()), count(reads), count(h.passed_read_counts[q][l])).unwrap();
        }
    }
    if !populated {
        empty_row(out, 4, "No basecalled length/Q-score observations.");
    }
    out.push_str("</tbody></table></div></details>");
}

fn length_quality_map(
    out: &mut String,
    histogram: &crate::metrics::LengthQualityHistogram,
    id: &str,
    title: &str,
    counts: &[Vec<u64>],
    maximum: u64,
) {
    figure(out, id, title);
    if !counts.iter().flatten().any(|&v| v != 0)
        || histogram.length_lower_bounds_bp.is_empty()
        || histogram.qscore_lower_bounds.is_empty()
    {
        empty_chart(out, "No basecalled observations in this subset.");
        return;
    }
    let columns = histogram.length_lower_bounds_bp.len();
    let rows = histogram.qscore_lower_bounds.len();
    let width = (RIGHT - LEFT) / columns as f64;
    let height = 14.0;
    let bottom = 48.0 + rows as f64 * height;
    svg_start(
        out,
        id,
        "Each cell counts reads, not bases. Exact counts and intervals are also available in the following table. Color uses log10(1 + read count), shared across both subsets.",
        560,
    );
    out.push_str(
        "<text class=\"axis-label\" x=\"80\" y=\"20\">Read Q-score · final row ≥ 50</text>",
    );
    for (q, row) in counts.iter().enumerate() {
        for (l, &reads) in row.iter().enumerate() {
            let x = LEFT + l as f64 * width;
            let y = bottom - (q + 1) as f64 * height;
            let intensity = (reads as f64).ln_1p() / (maximum.max(1) as f64).ln_1p();
            let label = format!(
                "Length {} bp; Q-score {}; {} reads",
                length_interval(
                    histogram.length_lower_bounds_bp[l],
                    histogram.length_lower_bounds_bp.get(l + 1).copied()
                ),
                quality_interval(
                    histogram.qscore_lower_bounds[q],
                    histogram.qscore_lower_bounds.get(q + 1).copied()
                ),
                count(reads)
            );
            write!(out, "<rect class=\"joint-cell\" x=\"{x:.2}\" y=\"{y:.2}\" width=\"{width:.2}\" height=\"{height}\" fill=\"hsl(225 45% {:.2}%)\" data-length-bin=\"{l}\" data-qscore-bin=\"{q}\" data-read-count=\"{reads}\" role=\"img\" aria-label=\"{}\"><title>{}</title></rect>", 97.0 - 65.0 * intensity, escape(&label), escape(&label)).unwrap();
        }
    }
    for (q, &bound) in histogram.qscore_lower_bounds.iter().enumerate().step_by(5) {
        let y = bottom - q as f64 * height - if q + 1 == rows { height / 2.0 } else { 0.0 };
        write!(
            out,
            "<text class=\"tick\" x=\"68\" y=\"{:.2}\" text-anchor=\"end\">{}{}</text>",
            y + 4.0,
            if q + 1 == rows { "≥ " } else { "" },
            decimal(bound)
        )
        .unwrap();
    }
    for (l, &bound) in histogram
        .length_lower_bounds_bp
        .iter()
        .enumerate()
        .step_by(4)
    {
        let overflow = l + 1 == columns;
        let x = LEFT + (l as f64 + if overflow { 0.5 } else { 0.0 }) * width;
        write!(
            out,
            "<text class=\"tick\" x=\"{x:.2}\" y=\"436\" text-anchor=\"middle\">{}{}</text>",
            if overflow { "≥ " } else { "" },
            compact(bound as f64)
        )
        .unwrap();
    }
    write!(out, "<rect class=\"histogram-overflow\" x=\"{:.2}\" y=\"48\" width=\"{width:.2}\" height=\"{:.2}\"/><rect class=\"histogram-overflow\" x=\"{LEFT}\" y=\"48\" width=\"{}\" height=\"{height}\"/><text class=\"axis-label\" x=\"397\" y=\"466\" text-anchor=\"middle\">Read length (bp, log₂ intervals; final column open-ended)</text>", RIGHT-width, rows as f64*height, RIGHT-LEFT).unwrap();
    for i in 0..64 {
        write!(
            out,
            "<rect x=\"{}\" y=\"500\" width=\"5\" height=\"12\" fill=\"hsl(225 45% {:.2}%)\"/>",
            80 + i * 5,
            97.0 - 65.0 * i as f64 / 63.0
        )
        .unwrap();
    }
    write!(out, "<text class=\"tick\" x=\"80\" y=\"530\">0 reads</text><text class=\"tick\" x=\"400\" y=\"530\" text-anchor=\"end\">{} reads</text><text class=\"axis-label\" x=\"80\" y=\"553\">Read count · log₁₀(1 + count), common scale</text></svg></div></figure>", count(maximum)).unwrap();
}

fn channels(out: &mut String, report: &Report) {
    section(
        out,
        "channels",
        "05",
        "Channel performance",
        "Ranked views use channel identifiers only; physical maps require explicit layout selection. Channels with only zero-length records have no mean Q-score.",
    );
    physical_heatmaps(out, report);
    figure(
        out,
        "channel-yield",
        "Channel yield, ranked from highest to lowest",
    );
    let mut ranked: Vec<_> = report.channels.iter().collect();
    ranked.sort_unstable_by_key(|c| (std::cmp::Reverse(c.total_yield_bp), c.channel));
    let max = ranked.first().map_or(0, |c| c.total_yield_bp);
    if max == 0 {
        empty_chart(out, "No basecalled channel yield available.");
    } else {
        let (scale, unit) = bp_scale(max);
        let (ymax, ticks) = plot_scale(max as f64 / scale, scale == 1.0);
        svg_start(
            out,
            "channel-yield",
            "Each point is one channel, ranked by yield; not a physical flowcell map.",
            294,
        );
        axes(out, ymax, ticks, &format!("Channel yield ({unit})"));
        let span = ranked.len().saturating_sub(1).max(1) as f64;
        for (i, c) in ranked.iter().enumerate() {
            let x = LEFT + i as f64 / span * (RIGHT - LEFT);
            let y = BOTTOM - c.total_yield_bp as f64 / scale / ymax * (BOTTOM - TOP);
            write!(out, "<circle class=\"channel-point\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"2\"><title>Channel {}: {} bp; {} basecalled reads; mean Q-score {}</title></circle>", c.channel, count(c.total_yield_bp), count(c.basecalled_reads), optional(c.mean_qscore, 1.0, "")).unwrap();
        }
        let ticks = ranked.len().saturating_sub(1).min(4);
        for i in 0..=ticks {
            let fraction = if ticks == 0 {
                0.0
            } else {
                i as f64 / ticks as f64
            };
            x_tick(
                out,
                LEFT + fraction * (RIGHT - LEFT),
                &count(1 + (fraction * span).round() as u64),
            );
        }
        out.push_str("<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Channel rank (descending yield)</text></svg></div></figure>");
    }
    let mean_lengths: Vec<_> = report
        .diagnostics
        .channel_read_length_distribution
        .iter()
        .map(|bin| {
            (
                length_interval(bin.lower_inclusive_bp, bin.upper_exclusive_bp),
                bin.channel_count,
                "primary",
            )
        })
        .collect();
    bars(
        out,
        "channel-length",
        "Mean read length by channel",
        "Channels (channels)",
        1.0,
        &mean_lengths,
    );
    out.push_str("<p class=\"annotation\">Each eligible channel counts once in its mean-length interval; channels without basecalled reads are excluded. This is a distribution of channel means, not individual read lengths. Exact all/passed channel means are in the table; per-channel medians are not calculated.</p>");
    out.push_str(r##"<div id="channel-controls" class="table-controls" hidden><label>Find channel <input id="channel-search" type="search" placeholder="Channel ID" autocomplete="off"></label><label>Sort by <select id="channel-sort"><option value="channel">Channel ID ↑</option><option value="yield">Yield ↓</option><option value="reads">Basecalled reads ↓</option><option value="qscore">Mean Q-score ↓</option><option value="meanlength">Mean length ↓</option><option value="passedlength">Mean passed length ↓</option></select></label><label>Rows <select id="channel-page-size"><option>25</option><option>50</option><option>100</option></select></label></div>
<div class="table-scroll channel-table"><table id="channel-table"><caption>All observed channels · exact counts and yield; default order is channel ID</caption><thead><tr><th scope="col">Channel</th><th scope="col">Records</th><th scope="col">Basecalled reads</th><th scope="col">Yield (bp)</th><th scope="col">Passed reads</th><th scope="col">Passed yield (bp)</th><th scope="col">Mean Q-score</th><th scope="col">Mean length (bp)</th><th scope="col">Mean passed length (bp)</th></tr></thead><tbody>"##);
    for c in &report.channels {
        write!(out, "<tr data-channel=\"{}\" data-yield=\"{}\" data-reads=\"{}\" data-qscore=\"{}\" data-meanlength=\"{}\" data-passedlength=\"{}\"><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            c.channel, c.total_yield_bp, c.basecalled_reads, c.mean_qscore.map(|v| v.to_string()).unwrap_or_default(), c.mean_read_length_bp.map(|v| v.to_string()).unwrap_or_default(), c.mean_passed_read_length_bp.map(|v| v.to_string()).unwrap_or_default(), c.channel, count(c.total_records), count(c.basecalled_reads), count(c.total_yield_bp), count(c.passed_reads), count(c.passed_yield_bp), optional(c.mean_qscore, 1.0, ""), optional(c.mean_read_length_bp, 1.0, ""), optional(c.mean_passed_read_length_bp, 1.0, "")).unwrap();
    }
    if report.channels.is_empty() {
        empty_row(out, 9, "No observed channels.");
    }
    out.push_str(r##"</tbody></table></div><div class="pagination" id="channel-pagination" hidden><span id="channel-status" role="status" aria-live="polite"></span><div><button id="channel-previous" type="button">Previous</button><button id="channel-next" type="button">Next</button></div></div><noscript><p class="annotation">JavaScript is disabled. All channels remain available in the scrollable table.</p></noscript>"##);
    channel_mux_table(out, report);
    out.push_str("</section>");
}

fn muxes(out: &mut String, report: &Report) {
    section(
        out,
        "mux",
        "06",
        "Mux performance",
        "Counts and yield by observed mux identifier, including record counts for zero-length entries.",
    );
    out.push_str("<div class=\"chart-grid\">");
    let reads: Vec<_> = report
        .muxes
        .iter()
        .map(|m| (format!("Mux {}", m.mux), m.basecalled_reads, "primary"))
        .collect();
    bars(
        out,
        "mux-reads",
        "Basecalled reads by mux",
        "Basecalled reads (reads)",
        1.0,
        &reads,
    );
    let yields: Vec<_> = report
        .muxes
        .iter()
        .map(|m| (format!("Mux {}", m.mux), m.total_yield_bp, "primary"))
        .collect();
    let (scale, unit) = bp_scale(
        report
            .muxes
            .iter()
            .map(|m| m.total_yield_bp)
            .max()
            .unwrap_or(0),
    );
    bars(
        out,
        "mux-yield",
        "Yield by mux",
        &format!("Yield ({unit})"),
        scale,
        &yields,
    );
    let passed: Vec<_> = report
        .muxes
        .iter()
        .map(|m| (format!("Mux {}", m.mux), m.passed_yield_bp, "passed"))
        .collect();
    bars(
        out,
        "mux-passed-yield",
        "Passed yield by mux",
        &format!("Yield ({unit})"),
        scale,
        &passed,
    );
    out.push_str("</div><p class=\"annotation\">Mux identifiers are used as observed, without assuming four values. Read counts and yields do not measure pore occupancy. The channel × mux breakdown is available in the channel section.</p><div class=\"table-scroll\"><table><caption>Exact mux totals</caption><thead><tr><th scope=\"col\">Mux</th><th scope=\"col\">Records</th><th scope=\"col\">Basecalled reads</th><th scope=\"col\">Yield (bp)</th><th scope=\"col\">Passed yield (bp)</th></tr></thead><tbody>");
    for m in &report.muxes {
        write!(
            out,
            "<tr><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            m.mux,
            count(m.total_records),
            count(m.basecalled_reads),
            count(m.total_yield_bp),
            count(m.passed_yield_bp)
        )
        .unwrap();
    }
    if report.muxes.is_empty() {
        empty_row(out, 5, "No observed muxes.");
    }
    out.push_str("</tbody></table></div></section>");
}

fn termination(out: &mut String, report: &Report) {
    section(
        out,
        "termination",
        "07",
        "Read termination reasons",
        "Unknown categories are preserved verbatim. A termination reason does not determine pass or fail status.",
    );
    out.push_str("<div class=\"table-scroll\"><table class=\"reason-table\"><caption>Termination categories; bars compare summary record counts</caption><thead><tr><th class=\"label\" scope=\"col\">End reason</th><th scope=\"col\">Summary records</th><th scope=\"col\">Basecalled reads</th><th scope=\"col\">Yield (bp)</th><th scope=\"col\">All records (%)</th><th scope=\"col\">Zero-length (%)</th><th scope=\"col\">Basecalled (%)</th><th scope=\"col\">Passed / basecalled (%)</th><th scope=\"col\">All yield (%)</th></tr></thead><tbody>");
    let max = report
        .end_reasons
        .iter()
        .map(|r| r.total_records)
        .max()
        .unwrap_or(1)
        .max(1);
    let reasons: std::collections::BTreeMap<_, _> = report
        .end_reasons
        .iter()
        .map(|r| (r.end_reason.as_str(), r))
        .collect();
    for diagnostic in &report.diagnostics.termination_reasons {
        let reason = reasons[diagnostic.end_reason.as_str()];
        write!(out, "<tr><th class=\"label\" scope=\"row\"><span class=\"reason-name\">{}</span></th><td class=\"bar-cell\"><span class=\"record-count\">{}</span><span class=\"table-bar\" style=\"--bar-width:{:.2}%\" aria-hidden=\"true\"></span></td><td>{}</td><td>{}</td>",
            escape(&reason.end_reason), count(reason.total_records), bar_percent(reason.total_records, max), count(reason.basecalled_reads), count(reason.total_yield_bp)).unwrap();
        for value in [
            diagnostic.summary_record_percent,
            diagnostic.zero_length_fraction.map(|v| v * 100.0),
            diagnostic.basecalled_read_fraction.map(|v| v * 100.0),
            diagnostic.passed_fraction_of_basecalled.map(|v| v * 100.0),
            diagnostic.total_yield_percent,
        ] {
            write!(out, "<td>{}</td>", optional(value, 1.0, "%")).unwrap();
        }
        out.push_str("</tr>");
    }
    if report.end_reasons.is_empty() {
        empty_row(out, 9, "No termination observations.");
    }
    out.push_str("</tbody></table></div><p class=\"annotation\">Sorted by record count, then reason name. Zero-length and basecalled percentages use records within each reason; passed percentage uses basecalled reads within that reason. All-record and all-yield percentages use run totals. Unavailable denominators are shown explicitly.</p></section>");
}

// Decorative proportions only; exact counts/yields remain separate text.
fn bar_percent(value: u64, maximum: u64) -> f64 {
    if maximum == 0 {
        0.0
    } else {
        (value as f64 / maximum as f64 * 100.0).clamp(0.0, 100.0)
    }
}

fn physical_heatmaps(out: &mut String, report: &Report) {
    let Some(layout) = &report.diagnostics.physical_layout else {
        out.push_str("<p class=\"annotation\">Physical channel maps are disabled. Select <code>--flowcell-layout promethion-3000</code> only for compatible channel numbering.</p>");
        return;
    };
    out.push_str("<h3>PromethION spatial yield · 3000-channel layout</h3><p class=\"annotation\">Explicitly selected EPI2ME Basic QC Tutorial map: twelve 25 × 10 blocks, forming 25 rows × 120 columns. Row and column indices are zero-based; orientation follows the reference, not a verified instrument-facing orientation. Unobserved channels are not evidence of dead or inactive pores. Exact observed values are also available in the channel table.</p>");
    let channels: std::collections::BTreeMap<_, _> =
        report.channels.iter().map(|c| (c.channel, c)).collect();
    for passed in [false, true] {
        let (id, title, hue) = if passed {
            (
                "spatial-passed-yield",
                "Passed yield per physical channel",
                164,
            )
        } else {
            (
                "spatial-total-yield",
                "Total basecalled yield per physical channel",
                228,
            )
        };
        let maximum = report
            .channels
            .iter()
            .map(|c| {
                if passed {
                    c.passed_yield_bp
                } else {
                    c.total_yield_bp
                }
            })
            .max()
            .unwrap_or(0);
        let (scale, unit) = bp_scale(maximum);
        figure(out, id, title);
        out.push_str(
            "<p class=\"scroll-hint\">Scroll map horizontally to view all 120 columns.</p>",
        );
        let high_lightness = if maximum == 0 { 96 } else { 42 };
        write!(out, "<div class=\"plot-scroll\" tabindex=\"0\" role=\"group\" aria-label=\"Scrollable physical channel heatmap\"><svg class=\"heatmap\" viewBox=\"0 0 1280 360\" role=\"group\" aria-labelledby=\"{id}-caption\"><defs><pattern id=\"{id}-unobserved\" width=\"4\" height=\"4\" patternUnits=\"userSpaceOnUse\"><rect width=\"4\" height=\"4\" fill=\"#f1f3f6\"/><path d=\"M0 0L4 4\" stroke=\"#cbd2df\" stroke-width=\"1\"/></pattern><linearGradient id=\"{id}-scale\"><stop stop-color=\"hsl({hue} 45% 96%)\"/><stop offset=\"1\" stop-color=\"hsl({hue} 45% {high_lightness}%)\"/></linearGradient></defs>").unwrap();
        for coordinate in &layout.channel_coordinates {
            let observed = channels.get(&coordinate.channel);
            let value = observed.map(|c| {
                if passed {
                    c.passed_yield_bp
                } else {
                    c.total_yield_bp
                }
            });
            let fill = value.map_or_else(
                || format!("url(#{id}-unobserved)"),
                |v| {
                    let fraction = if maximum == 0 {
                        0.0
                    } else {
                        v as f64 / maximum as f64
                    };
                    format!("hsl({hue} 45% {:.2}%)", 96.0 - fraction * 54.0)
                },
            );
            let description = match (observed, value) {
                (Some(c), Some(v)) => format!(
                    "Channel {} · row {}, column {}: {} bp {}; {} summary records; {} basecalled reads",
                    coordinate.channel,
                    coordinate.row,
                    coordinate.column,
                    count(v),
                    if passed {
                        "passed yield"
                    } else {
                        "total yield"
                    },
                    count(c.total_records),
                    count(c.basecalled_reads)
                ),
                _ => format!(
                    "Channel {} · row {}, column {}: not observed in summary; yield unavailable",
                    coordinate.channel, coordinate.row, coordinate.column
                ),
            };
            write!(out, "<rect class=\"heatmap-cell\" data-channel=\"{}\" data-yield-bp=\"{}\" x=\"{}\" y=\"{}\" width=\"9\" height=\"9\" fill=\"{fill}\" role=\"img\" aria-labelledby=\"{id}-c{}\"><title id=\"{id}-c{}\">{}</title></rect>", coordinate.channel, value.map(|v| v.to_string()).unwrap_or_default(), 48 + coordinate.column * 10, 28 + coordinate.row * 10, coordinate.channel, coordinate.channel, escape(&description)).unwrap();
        }
        for row in [0, 5, 10, 15, 20, 24] {
            write!(
                out,
                "<text class=\"tick\" x=\"38\" y=\"{}\" text-anchor=\"end\">{row}</text>",
                36 + row * 10
            )
            .unwrap();
        }
        for column in [0, 20, 40, 60, 80, 100, 119] {
            write!(
                out,
                "<text class=\"tick\" x=\"{}\" y=\"296\" text-anchor=\"middle\">{column}</text>",
                52 + column * 10
            )
            .unwrap();
        }
        write!(out, "<text class=\"axis-label\" x=\"48\" y=\"17\">Row</text><text class=\"axis-label\" x=\"1248\" y=\"317\" text-anchor=\"end\">Column</text><rect x=\"48\" y=\"317\" width=\"240\" height=\"10\" fill=\"url(#{id}-scale)\"/><text class=\"tick\" x=\"48\" y=\"349\">0 {unit}</text><text class=\"tick\" x=\"288\" y=\"349\" text-anchor=\"end\">{} {unit}</text><rect x=\"360\" y=\"317\" width=\"14\" height=\"14\" fill=\"url(#{id}-unobserved)\"/><text class=\"tick\" x=\"382\" y=\"329\">Not observed · unavailable yield</text><rect x=\"760\" y=\"317\" width=\"14\" height=\"14\" fill=\"hsl({hue} 45% 96%)\" stroke=\"#aab4c8\"/><text class=\"tick\" x=\"782\" y=\"329\">Observed · zero yield</text></svg></div></figure>", decimal(maximum as f64 / scale)).unwrap();
    }
}

fn channel_mux_table(out: &mut String, report: &Report) {
    out.push_str(r##"<details><summary>View channel × mux yield breakdown</summary><p class="annotation">Observed channel/mux pairs only. Bars compare total yield across pairs. Counts and yield are summary-derived, not pore occupancy.</p>
<div id="channel-mux-controls" class="table-controls" hidden><label>Find channel <input id="channel-mux-search" type="search" placeholder="Channel ID" autocomplete="off"></label><label>Sort by <select id="channel-mux-sort"><option value="channel">Channel ID ↑</option><option value="mux">Mux ID ↑</option><option value="yield">Yield ↓</option><option value="reads">Basecalled reads ↓</option></select></label><label>Rows <select id="channel-mux-page-size"><option>25</option><option>50</option><option>100</option></select></label></div>
<div class="table-scroll"><table id="channel-mux-table"><caption>Channel × mux · exact counts and bp; default order is channel then mux</caption><thead><tr><th scope="col">Channel</th><th scope="col">Mux</th><th scope="col">Records</th><th scope="col">Basecalled reads</th><th scope="col">Zero-length records</th><th scope="col">Yield (bp)</th><th scope="col">Passed reads</th><th scope="col">Passed yield (bp)</th></tr></thead><tbody>"##);
    let maximum = report
        .diagnostics
        .channel_mux
        .iter()
        .map(|m| m.total_yield_bp)
        .max()
        .unwrap_or(1)
        .max(1);
    for m in &report.diagnostics.channel_mux {
        write!(out, "<tr data-channel=\"{}\" data-mux=\"{}\" data-yield=\"{}\" data-reads=\"{}\"><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"bar-cell\"><span class=\"record-count\">{}</span><span class=\"table-bar\" style=\"--bar-width:{:.2}%\" aria-hidden=\"true\"></span></td><td>{}</td><td>{}</td></tr>", m.channel, m.mux, m.total_yield_bp, m.basecalled_reads, m.channel, m.mux, count(m.total_records), count(m.basecalled_reads), count(m.zero_length_records), count(m.total_yield_bp), bar_percent(m.total_yield_bp, maximum), count(m.passed_reads), count(m.passed_yield_bp)).unwrap();
    }
    if report.diagnostics.channel_mux.is_empty() {
        empty_row(out, 8, "No observed channel/mux pairs.");
    }
    out.push_str(r##"</tbody></table></div><div class="pagination" id="channel-mux-pagination" hidden><span id="channel-mux-status" role="status" aria-live="polite"></span><div><button id="channel-mux-previous" type="button">Previous</button><button id="channel-mux-next" type="button">Next</button></div></div></details>"##);
}

fn sequencing_speed(out: &mut String, report: &Report) {
    section(
        out,
        "speed",
        "08",
        "Sequencing speed proxy",
        "Basecalled length / template_duration, in bp/s. This is not a direct motor-speed or raw-signal measurement.",
    );
    let Some(speed) = &report.diagnostics.sequencing_speed else {
        out.push_str("<p class=\"notice\">Sequencing speed is not available: the input has no <code>template_duration</code> column. <code>duration</code> is not substituted.</p></section>");
        return;
    };
    write!(out, "<div class=\"stat-line\"><span>Eligible reads <strong data-metric=\"speed_eligible_reads\" data-value=\"{}\">{}</strong></span><span>Mean per-read speed <strong data-metric=\"mean_speed_bp_per_second\" data-value=\"{}\">{}</strong></span></div><p class=\"annotation\">Only positive-length reads with a finite positive template duration and a finite ratio are eligible. Blank or invalid optional durations are excluded from speed only; no duration fallback or speed threshold is applied. Mean is arithmetic per eligible read. Intervals are lower-inclusive and upper-exclusive; the final interval is open-ended.</p>", speed.summary.eligible_reads, count(speed.summary.eligible_reads), speed.summary.mean_bp_per_second.map(|v| v.to_string()).unwrap_or_default(), optional(speed.summary.mean_bp_per_second, 1.0, "bp/s")).unwrap();
    if speed.summary.eligible_reads == 0 {
        out.push_str("<p class=\"notice\">No eligible sequencing speed observations. Speed is not available.</p></section>");
        return;
    }
    out.push_str("<div class=\"chart-grid\">");
    let rows: Vec<_> = speed
        .histogram
        .iter()
        .map(|b| {
            (
                format!(
                    "{} bp/s",
                    quality_interval(
                        b.lower_inclusive_bp_per_second,
                        b.upper_exclusive_bp_per_second
                    )
                ),
                b.read_count,
                "primary",
            )
        })
        .collect();
    bars(
        out,
        "speed-distribution",
        "Read speed proxy distribution",
        "Eligible reads (reads)",
        1.0,
        &rows,
    );
    diagnostic_time_plot(out, report, true);
    out.push_str("</div><p class=\"annotation\">Mean speed is attributed to read start time in 15-minute bins. Unavailable or missing intervals are not joined by lines.</p></section>");
}

// The two observed diagnostics share time coordinates and gap handling.
fn diagnostic_time_plot(out: &mut String, report: &Report, speed: bool) {
    let (id, title, axis) = if speed {
        (
            "speed-time",
            "Mean read speed proxy over time",
            "Mean read speed (bp/s)",
        )
    } else {
        (
            "channel-activity",
            "Channels producing basecalled reads over time",
            "Basecalled-producing channels (channels)",
        )
    };
    let points: Vec<_> = if speed {
        report
            .diagnostics
            .sequencing_speed
            .as_ref()
            .map(|s| {
                s.time_bins
                    .iter()
                    .map(|b| (b.start_seconds, b.summary.mean_bp_per_second))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        report
            .diagnostics
            .channel_activity
            .iter()
            .map(|b| {
                (
                    b.start_seconds,
                    Some(b.basecalled_producing_channels as f64),
                )
            })
            .collect()
    };
    time_series_plot(
        out,
        report,
        id,
        title,
        axis,
        &[TimeSeries {
            label: if speed {
                "Mean read speed"
            } else {
                "Observed channels"
            },
            color: "primary",
            points,
        }],
        !speed,
    );
}

struct TimeSeries {
    label: &'static str,
    color: &'static str,
    points: Vec<(u64, Option<f64>)>,
}

// Shared only by start-time diagnostics: common axes, bin midpoints and gaps.
fn time_series_plot(
    out: &mut String,
    report: &Report,
    id: &str,
    title: &str,
    axis: &str,
    series: &[TimeSeries],
    integer_ticks: bool,
) {
    figure(out, id, title);
    let maximum = series
        .iter()
        .flat_map(|s| s.points.iter().filter_map(|(_, value)| *value))
        .reduce(f64::max);
    let Some(maximum) = maximum else {
        empty_chart(
            out,
            "No eligible basecalled observations in these intervals.",
        );
        return;
    };
    let maximum = if maximum == 0.0 { 1.0 } else { maximum };
    // Finite extreme ratios remain valid data; avoid overflow in rounded axes.
    let (ymax, ticks) = if !(1e-200..1e300).contains(&maximum) {
        (maximum, 4)
    } else {
        plot_scale(maximum, integer_ticks)
    };
    let end = report.time_bins.last().map_or(900, |b| b.end_seconds) as f64;
    svg_start(out, id, title, 294);
    axes(out, ymax, ticks, axis);
    for i in 0..=4 {
        x_tick(
            out,
            LEFT + (RIGHT - LEFT) * i as f64 / 4.0,
            &decimal(end / 3600.0 * i as f64 / 4.0),
        );
    }
    out.push_str("<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Time from run start (h)</text>");
    for s in series {
        let mut path = String::new();
        let mut previous = None;
        for &(start, value) in &s.points {
            let Some(value) = value else {
                previous = None;
                continue;
            };
            let x = LEFT + (start as f64 + 450.0) / end * (RIGHT - LEFT);
            let y = BOTTOM - value / ymax * (BOTTOM - TOP);
            let command = if previous.is_some_and(|last| start - last == 900) {
                'L'
            } else {
                'M'
            };
            write!(path, " {command} {x:.2} {y:.2}").unwrap();
            write!(out, "<circle class=\"{}\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"3\" data-value=\"{value}\"><title>[{}, {}) seconds · {}: {} ({})</title></circle>", s.color, start, start + 900, escape(s.label), decimal(value), escape(axis)).unwrap();
            previous = Some(start);
        }
        let class = if s.color == "passed" {
            "passed-line"
        } else {
            "cumulative-line"
        };
        write!(out, "<path class=\"{class}\" d=\"{path}\"/>").unwrap();
    }
    out.push_str("</svg></div>");
    if series.len() > 1 {
        out.push_str("<p class=\"annotation\"><span class=\"legend all\">All basecalled reads · solid</span><span class=\"legend pass\">Passed reads · dashed</span></p>");
    }
    out.push_str("</figure>");
}

fn figure(out: &mut String, id: &str, title: &str) {
    write!(
        out,
        "<figure><figcaption id=\"{id}-caption\">{}</figcaption>",
        escape(title)
    )
    .unwrap();
}

fn svg_start(out: &mut String, id: &str, description: &str, height: usize) {
    write!(out, "<p class=\"scroll-hint\">Scroll chart horizontally for full axes and values.</p><div class=\"plot-scroll\" tabindex=\"0\" role=\"group\" aria-label=\"Scrollable chart\"><svg class=\"plot\" viewBox=\"0 0 760 {height}\" role=\"img\" aria-labelledby=\"{id}-caption\"><title>{}</title>", escape(description)).unwrap();
}

fn empty_chart(out: &mut String, message: &str) {
    write!(out, "<p class=\"empty-chart\">{message}</p></figure>").unwrap();
}

fn empty_row(out: &mut String, columns: u8, message: &str) {
    write!(
        out,
        "<tr><td class=\"empty-row\" colspan=\"{columns}\">{message}</td></tr>"
    )
    .unwrap();
}

fn axes(out: &mut String, ymax: f64, ticks: usize, label: &str) {
    for i in 0..=ticks {
        let y = BOTTOM - (BOTTOM - TOP) * i as f64 / ticks as f64;
        write!(out, "<line class=\"grid-line\" x1=\"{LEFT}\" x2=\"{RIGHT}\" y1=\"{y:.2}\" y2=\"{y:.2}\"/><text class=\"tick\" x=\"68\" y=\"{:.2}\" text-anchor=\"end\">{}</text>", y + 4.0, compact(ymax * (i as f64 / ticks as f64))).unwrap();
    }
    write!(
        out,
        "<text class=\"axis-label\" x=\"{LEFT}\" y=\"17\">{}</text>",
        escape(label)
    )
    .unwrap();
}

fn x_tick(out: &mut String, x: f64, label: &str) {
    write!(
        out,
        "<text class=\"tick\" x=\"{x:.2}\" y=\"256\" text-anchor=\"middle\">{}</text>",
        escape(label)
    )
    .unwrap();
}

// Categorical horizontal bars for the report's existing histogram/group values.
fn bars(
    out: &mut String,
    id: &str,
    title: &str,
    axis: &str,
    scale: f64,
    rows: &[(String, u64, &str)],
) {
    figure(out, id, title);
    let max = rows.iter().map(|r| r.1).max().unwrap_or(0);
    if max == 0 {
        empty_chart(out, "No basecalled observations available.");
        return;
    }
    let height = 80 + rows.len() * 34;
    let baseline = height - 50;
    svg_start(out, id, title, height);
    let (xmax, ticks) = plot_scale(max as f64 / scale, scale == 1.0);
    for i in 0..=ticks {
        let x = 165.0 + 480.0 * i as f64 / ticks as f64;
        write!(out, "<line class=\"grid-line\" x1=\"{x:.2}\" x2=\"{x:.2}\" y1=\"18\" y2=\"{baseline}\"/><text class=\"tick\" x=\"{x:.2}\" y=\"{}\" text-anchor=\"middle\">{}</text>", baseline + 20, compact(xmax * i as f64 / ticks as f64)).unwrap();
    }
    for (i, (label, value, color)) in rows.iter().enumerate() {
        let y = 25 + i * 34;
        let raw_unit = if axis.starts_with("Yield") {
            "bp"
        } else if axis.ends_with("(channels)") {
            "channels"
        } else {
            "reads"
        };
        write!(out, "<text class=\"tick category\" x=\"154\" y=\"{}\" text-anchor=\"end\">{}</text><rect class=\"{color}\" x=\"165\" y=\"{y}\" width=\"{:.2}\" height=\"18\" rx=\"2\" data-value=\"{value}\"><title>{}: {} {raw_unit}</title></rect><text class=\"tick\" x=\"736\" y=\"{}\" text-anchor=\"end\">{}</text>",
            y + 13, escape(label), *value as f64 / scale / xmax * 480.0, escape(label), count(*value), y + 13, compact(*value as f64 / scale)).unwrap();
    }
    write!(out, "<text class=\"axis-label\" x=\"405\" y=\"{}\" text-anchor=\"middle\">{}</text></svg></div></figure>", height - 7, escape(axis)).unwrap();
}

// Presentation-only scale rounding; histogram counts and QC values stay intact.
fn plot_scale(max: f64, integer_ticks: bool) -> (f64, usize) {
    let raw_step = max / 4.0;
    let power = 10.0_f64.powf(raw_step.log10().floor());
    let factor = raw_step / power;
    let rounded = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .find(|v| *v >= factor)
        .unwrap_or(10.0);
    let step = (rounded * power).max(if integer_ticks {
        1.0
    } else {
        f64::MIN_POSITIVE
    });
    let ticks = (max / step).ceil() as usize;
    (step * ticks as f64, ticks)
}

fn length_interval(lower: u64, upper: Option<u64>) -> String {
    upper.map_or_else(
        || format!("≥ {}", count(lower)),
        |u| format!("[{}, {})", count(lower), count(u)),
    )
}

fn quality_interval(lower: f64, upper: Option<f64>) -> String {
    upper.map_or_else(
        || format!("≥ {}", decimal(lower)),
        |u| format!("[{}, {})", decimal(lower), decimal(u)),
    )
}

fn optional(value: Option<f64>, scale: f64, unit: &str) -> String {
    value.map_or_else(
        || "Not available".to_owned(),
        |v| {
            format!("{} {unit}", decimal(v / scale))
                .trim_end()
                .to_owned()
        },
    )
}

fn bp_scale(value: u64) -> (f64, &'static str) {
    match value {
        1_000_000_000.. => (1e9, "Gb"),
        1_000_000.. => (1e6, "Mb"),
        1_000.. => (1e3, "kb"),
        _ => (1.0, "bp"),
    }
}

fn compact(value: f64) -> String {
    if value >= 1e9 {
        format!("{}G", decimal(value / 1e9))
    } else if value >= 1e6 {
        format!("{}M", decimal(value / 1e6))
    } else if value >= 1e3 {
        format!("{}k", decimal(value / 1e3))
    } else {
        decimal(value)
    }
}

fn decimal(value: f64) -> String {
    if value != 0.0 && !(1e-9..1e9).contains(&value.abs()) {
        return format!("{value:.3e}");
    }
    let digits = if value.abs() >= 1.0 { 2 } else { 9 };
    format!("{value:.digits$}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn count(value: u64) -> String {
    let text = value.to_string();
    let mut out = String::new();
    for (i, c) in text.chars().enumerate() {
        if i > 0 && (text.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorative_bar_widths_are_finite_and_clamped() {
        for (value, maximum, expected) in [
            (0, 0, 0.0),
            (1, 0, 0.0),
            (0, 10, 0.0),
            (5, 10, 50.0),
            (10, 10, 100.0),
            (20, 10, 100.0),
            (u64::MAX, u64::MAX, 100.0),
        ] {
            assert_eq!(bar_percent(value, maximum), expected);
        }
    }

    #[test]
    fn absent_histograms_and_channels_render_valid_empty_states() {
        let mut report = crate::metrics::Aggregator::default().finish(None);
        report.read_length_distribution.clear();
        report.quality.histogram.clear();
        let html = render(&report);
        assert!(html.contains("No observed channels."));
        assert!(html.contains("No histogram intervals."));
        assert!(html.ends_with("</body></html>\n"));
    }

    #[test]
    fn display_units_preserve_small_nonzero_yields_and_unavailable_values() {
        assert_eq!(decimal(1.0 / 1e9), "0.000000001");
        assert_eq!(decimal(0.0), "0");
        assert_eq!(optional(None, 1.0, "kb"), "Not available");
        assert_eq!(optional(Some(25.0), 1000.0, "kb"), "0.025 kb");
        assert_eq!(count(u64::MAX), "18,446,744,073,709,551,615");
        assert_eq!(escape("<&>\"'"), "&lt;&amp;&gt;&quot;&#39;");
    }

    #[test]
    fn count_axes_have_integer_ticks_and_cover_the_observed_maximum() {
        for maximum in [1.0, 2.0, 3.0, 7.0, 999.0, u64::MAX as f64] {
            let (upper, ticks) = plot_scale(maximum, true);
            assert!(upper >= maximum);
            assert!((1..=5).contains(&ticks));
            assert_eq!((upper / ticks as f64).fract(), 0.0);
        }
    }
}
