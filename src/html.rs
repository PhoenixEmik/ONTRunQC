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
<a href="#termination">07 <span>Termination reasons</span></a>
<div class="nav-note">One run.<br>Summary-derived QC.<br>Available offline.</div></nav><main>"##);
    overview(&mut out, report);
    sequencing_yield(&mut out, report);
    lengths(&mut out, report);
    quality(&mut out, report);
    channels(&mut out, report);
    muxes(&mut out, report);
    termination(&mut out, report);
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
    out.push_str("</div><details><summary>View exact interval values</summary><div class=\"table-scroll\"><table><caption>Occupied time bins; omitted intervals contain no records.</caption><thead><tr><th class=\"label\" scope=\"col\">Start–end (s)</th><th scope=\"col\">Records</th><th scope=\"col\">Basecalled reads</th><th scope=\"col\">Yield (bp)</th><th scope=\"col\">Cumulative (bp)</th></tr></thead><tbody>");
    for bin in &report.time_bins {
        write!(
            out,
            "<tr><th class=\"label\" scope=\"row\">{}–{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            count(bin.start_seconds),
            count(bin.end_seconds),
            count(bin.total_records),
            count(bin.basecalled_reads),
            count(bin.yield_bp),
            count(bin.cumulative_yield_bp)
        )
        .unwrap();
    }
    if report.time_bins.is_empty() {
        empty_row(out, 5, "No time-bin observations.");
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
    out.push_str("</tbody></table></div></details></section>");
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
    out.push_str("</tbody></table></div></details></section>");
}

fn channels(out: &mut String, report: &Report) {
    section(
        out,
        "channels",
        "05",
        "Channel performance",
        "Channel identifiers carry no assumed physical position. Channels with only zero-length records have no mean Q-score.",
    );
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
    out.push_str(r##"<div id="channel-controls" class="table-controls" hidden><label>Find channel <input id="channel-search" type="search" placeholder="Channel ID" autocomplete="off"></label><label>Sort by <select id="channel-sort"><option value="channel">Channel ID ↑</option><option value="yield">Yield ↓</option><option value="reads">Basecalled reads ↓</option><option value="qscore">Mean Q-score ↓</option></select></label><label>Rows <select id="channel-page-size"><option>25</option><option>50</option><option>100</option></select></label></div>
<div class="table-scroll channel-table"><table id="channel-table"><caption>All observed channels · exact counts and yield; default order is channel ID</caption><thead><tr><th scope="col">Channel</th><th scope="col">Records</th><th scope="col">Basecalled reads</th><th scope="col">Yield (bp)</th><th scope="col">Passed reads</th><th scope="col">Passed yield (bp)</th><th scope="col">Mean Q-score</th></tr></thead><tbody>"##);
    for c in &report.channels {
        write!(out, "<tr data-channel=\"{}\" data-yield=\"{}\" data-reads=\"{}\" data-qscore=\"{}\"><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            c.channel, c.total_yield_bp, c.basecalled_reads, c.mean_qscore.map(|v| v.to_string()).unwrap_or_default(), c.channel, count(c.total_records), count(c.basecalled_reads), count(c.total_yield_bp), count(c.passed_reads), count(c.passed_yield_bp), optional(c.mean_qscore, 1.0, "")).unwrap();
    }
    if report.channels.is_empty() {
        empty_row(out, 7, "No observed channels.");
    }
    out.push_str(r##"</tbody></table></div><div class="pagination" id="channel-pagination" hidden><span id="channel-status" role="status" aria-live="polite"></span><div><button id="channel-previous" type="button">Previous</button><button id="channel-next" type="button">Next</button></div></div><noscript><p class="annotation">JavaScript is disabled. All channels remain available in the scrollable table.</p></noscript></section>"##);
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
    out.push_str("</div><div class=\"table-scroll\"><table><caption>Exact mux totals</caption><thead><tr><th scope=\"col\">Mux</th><th scope=\"col\">Records</th><th scope=\"col\">Basecalled reads</th><th scope=\"col\">Yield (bp)</th><th scope=\"col\">Passed yield (bp)</th></tr></thead><tbody>");
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
    out.push_str("<div class=\"table-scroll\"><table class=\"reason-table\"><caption>Termination categories; bars compare summary record counts</caption><thead><tr><th class=\"label\" scope=\"col\">End reason</th><th scope=\"col\">Summary records</th><th scope=\"col\">Basecalled reads</th><th scope=\"col\">Yield (bp)</th></tr></thead><tbody>");
    let max = report
        .end_reasons
        .iter()
        .map(|r| r.total_records)
        .max()
        .unwrap_or(1)
        .max(1);
    for reason in &report.end_reasons {
        write!(out, "<tr><th class=\"label\" scope=\"row\"><span class=\"reason-name\">{}</span></th><td class=\"bar-cell\"><span class=\"record-count\">{}</span><svg class=\"inline-bar\" viewBox=\"0 0 100 4\" preserveAspectRatio=\"none\" aria-hidden=\"true\"><rect class=\"bar-track\" width=\"100\" height=\"4\"/><rect class=\"bar-fill\" width=\"{:.2}\" height=\"4\"/></svg></td><td>{}</td><td>{}</td></tr>",
            escape(&reason.end_reason), count(reason.total_records), 100.0 * reason.total_records as f64 / max as f64, count(reason.basecalled_reads), count(reason.total_yield_bp)).unwrap();
    }
    if report.end_reasons.is_empty() {
        empty_row(out, 4, "No termination observations.");
    }
    out.push_str("</tbody></table></div></section>");
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
        write!(out, "<line class=\"grid-line\" x1=\"{LEFT}\" x2=\"{RIGHT}\" y1=\"{y:.2}\" y2=\"{y:.2}\"/><text class=\"tick\" x=\"68\" y=\"{:.2}\" text-anchor=\"end\">{}</text>", y + 4.0, compact(ymax * i as f64 / ticks as f64)).unwrap();
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
