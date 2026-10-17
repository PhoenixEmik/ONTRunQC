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
"##);
    if report.collection.is_some() {
        out.push_str(r##"<a href="#overview">01 Pooled overview</a><a href="#comparisons">02 Run comparisons</a><a href="#length">03 Pooled length</a><a href="#quality">04 Pooled quality</a><a href="#qcutoff">09 Pooled Q-cutoff</a><div class="nav-note">Independent run/source timelines.<br>Available offline.</div></nav><main>"##);
    } else {
        out.push_str(r##"<a href="#overview">01 Run overview</a><a href="#yield">02 Sequencing yield</a><a href="#length">03 Read length</a><a href="#quality">04 Read quality</a><a href="#channels">05 Channels</a><a href="#mux">06 Mux performance</a><a href="#termination">07 Termination reasons</a><a href="#speed">08 Sequencing speed</a><a href="#qcutoff">09 Q-cutoff comparison</a><div class="nav-note">One run.<br>Summary-derived QC.<br>Available offline.</div></nav><main>"##);
    }
    overview(&mut out, report);
    if let Some(hours) = report.presentation.mux_interval_hours {
        write!(out, "<p class=\"annotation\">Dashed mux-reference markers every {} hours are user configuration, not measured MinKNOW events.</p>", decimal(hours)).unwrap();
    }
    if report.collection.is_some() {
        combined_comparisons(&mut out, report);
    } else {
        sequencing_yield(&mut out, report);
    }
    lengths(&mut out, report);
    quality(&mut out, report);
    if report.collection.is_none() {
        channels(&mut out, report);
        muxes(&mut out, report);
        termination(&mut out, report);
        sequencing_speed(&mut out, report);
    }
    qscore_comparison(&mut out, report);
    write!(out, "<footer>PoreGauge {} · JSON schema {} · Post-sequencing QC from sequencing summaries. No automatic QC judgment.</footer></main></div><script>",
        escape(report.tool.version), report.schema_version).unwrap();
    out.push_str(SCRIPT);
    out.push_str("</script></body></html>\n");
    out
}

fn combined_comparisons(out: &mut String, report: &Report) {
    let collection = report.collection.as_ref().unwrap();
    section(
        out,
        "comparisons",
        "02",
        "Run / source comparisons",
        "Pooled metrics use exact combined observations. Timelines remain relative to each run's own start; yields are absolute, never normalized.",
    );
    out.push_str("<p class=\"annotation\">Source labels are deterministic output identifiers, not invented flowcell IDs. Matching run IDs identify chunks; a physical flowcell may have multiple runs. Instrument-local channels and muxes are available only in individual reports.</p><div class=\"table-scroll\"><table><caption>Inputs contributing to pooled statistics</caption><thead><tr><th class=\"label\">Source</th><th class=\"label\">Run ID</th><th class=\"label\">Flowcell ID</th><th class=\"label\">Read representation</th><th>Files</th><th>Basecalled reads</th><th>Yield (bp)</th><th>N50 (bp)</th></tr></thead><tbody>");
    for r in &collection.runs {
        write!(out, "<tr><th class=\"label\">{}</th><td class=\"label\">{}</td><td class=\"label\">{}</td><td class=\"label\">{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", escape(&r.source.source_id), escape(r.run_id.as_deref().unwrap_or("Unavailable · source only")), escape(r.source.flow_cell_id.as_deref().unwrap_or("Unavailable")), if r.input.read_representation == Some(crate::input::ReadRepresentation::TwoD) { "2D/1D2" } else { "Template" }, r.source.input_files, count(r.metrics.basecalled_reads), count(r.metrics.total_yield_bp), r.metrics.n50_bp.map(count).unwrap_or_else(|| "Not available".into())).unwrap();
    }
    out.push_str("</tbody></table></div>");
    for selected in [false, true] {
        let population = if selected {
            format!("Q ≥ {}", cutoff_value(report.comparison.qscore_cutoff))
        } else {
            "All basecalled reads".into()
        };
        for quality in [false, true] {
            comparison_distribution(out, report, selected, quality, &population);
        }
        for kind in [0, 1, 2] {
            let (name, title, axis) = match kind {
                0 => ("yield-time", "Cumulative yield by run", "Yield (Gb)"),
                1 => (
                    "length-time",
                    "Mean read length by run",
                    "Mean read length (kb)",
                ),
                _ => (
                    "quality-time",
                    "Mean Q-score by run",
                    "Arithmetic mean read Q-score",
                ),
            };
            let series: Vec<_> = collection
                .runs
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let points = if selected {
                        r.selected_time_bins
                            .iter()
                            .map(|b| {
                                (
                                    b.start_seconds,
                                    match kind {
                                        0 => Some(b.cumulative_yield_bp as f64 / 1e9),
                                        1 => b.mean_read_length_bp.map(|v| v / 1000.0),
                                        _ => b.mean_qscore,
                                    },
                                )
                            })
                            .collect()
                    } else {
                        r.time_bins
                            .iter()
                            .map(|b| {
                                (
                                    b.start_seconds,
                                    match kind {
                                        0 => Some(b.cumulative_yield_bp as f64 / 1e9),
                                        1 => b.mean_read_length_bp.map(|v| v / 1000.0),
                                        _ => b.mean_qscore,
                                    },
                                )
                            })
                            .collect()
                    };
                    TimeSeries {
                        label: &r.source.source_id,
                        color: run_color(i),
                        bin_offset_seconds: if kind == 0 { 900 } else { 450 },
                        points,
                    }
                })
                .collect();
            time_series_plot(
                out,
                report,
                &format!("combined-{name}-{selected}"),
                &format!("{title} · {population}"),
                axis,
                &series,
                false,
            );
        }
        comparison_yield_length(out, report, selected, &population);
    }
    fitted_trend_plot(out, report, false, false);
    fitted_trend_plot(out, report, false, true);
    fitted_trend_plot(out, report, true, false);
    fitted_trend_plot(out, report, true, true);
    if report.minionqc_density.is_some() {
        out.push_str("<p class=\"annotation\">R-grid density mode: Gaussian bw.nrd0, shared transformed scale domain, 512 mass/evaluation bins, extension of four bandwidths and direct discrete convolution equivalent to zero-padded FFT convolution. Q-score includes zero-length summary records; log-length density excludes them. Plotted-domain area may be below one because tails lie outside the observed range. Numerically validated against pinned R 4.4.1 / ggplot2 3.5.1 on public and synthetic fixtures.</p>");
    }
    out.push_str("<p class=\"annotation\">Legacy distribution estimates retained in JSON use Gaussian KDE with R-compatible bw.nrd0 bandwidth and bounded linear binning, normalized within each population. Length density uses log10(bp). Means retain unsmoothed 15-minute bins; gaps are not connected. Cumulative points attribute the entire bin's yield to its read-start interval. Color assignments are consistent across charts (palette repeats after eight sources). Exact histogram counts, thresholds and interval denominators remain in JSON.</p></section>");
}

fn run_color(index: usize) -> &'static str {
    [
        "run0", "run1", "run2", "run3", "run4", "run5", "run6", "run7",
    ][index % 8]
}

fn comparison_distribution(
    out: &mut String,
    report: &Report,
    selected: bool,
    quality: bool,
    population: &str,
) {
    let population = if report.minionqc_density.is_some() {
        if selected {
            format!(
                "Q ≥ {} summary records",
                cutoff_value(report.comparison.qscore_cutoff)
            )
        } else {
            "All summary records".into()
        }
    } else {
        population.to_owned()
    };
    let runs = &report.collection.as_ref().unwrap().runs;
    let id = format!("combined-distribution-{quality}-{selected}");
    let title = format!(
        "{} density by run · {population}",
        if quality { "Q-score" } else { "Read length" }
    );
    figure(out, &id, &title);
    let estimates: Vec<_> = runs
        .iter()
        .map(|r| {
            let estimates = r.minionqc_density.as_ref().unwrap_or(&r.distributions);
            match (quality, selected) {
                (false, false) => estimates.length_log10.as_ref(),
                (false, true) => estimates.selected_length_log10.as_ref(),
                (true, false) => estimates.qscore.as_ref(),
                (true, true) => estimates.selected_qscore.as_ref(),
            }
        })
        .collect();
    let points: Vec<_> = estimates.iter().flatten().flat_map(|e| &e.points).collect();
    let Some(xmin) = points.iter().map(|p| p.x).reduce(f64::min) else {
        empty_chart(
            out,
            "Density unavailable: no observations or a nonrepresentable plotting range.",
        );
        return;
    };
    let xmax = points.iter().map(|p| p.x).reduce(f64::max).unwrap();
    let constant = xmax == xmin;
    let maximum = points
        .iter()
        .map(|p| p.density)
        .reduce(f64::max)
        .unwrap_or(0.0);
    if maximum <= 0.0 {
        empty_chart(out, "Density unavailable for this plotting range.");
        return;
    }
    // Expand only the display axis for a constant statistical domain. The R
    // grid stays unchanged; show its evaluated value as a point, not a curve.
    let padding = (xmin.abs() * 0.05).max(0.5);
    let (xmin, xmax) = if constant {
        (xmin - padding, xmax + padding)
    } else {
        (xmin, xmax)
    };
    if !xmin.is_finite() || !xmax.is_finite() || xmax <= xmin {
        empty_chart(out, "Density unavailable for this plotting range.");
        return;
    }
    svg_start(
        out,
        &id,
        if report.minionqc_density.is_some() {
            "R 4.4.1 density: bw.nrd0, 512 mass bins, four-bandwidth extension and discrete convolution on a shared domain. Length is transformed before density estimation; zero lengths are omitted."
        } else {
            "Gaussian KDE: bw.nrd0 bandwidth, type-7 quartiles, 2048 linear bins and 512 display points. Length density uses log10(bp) space."
        },
        294,
    );
    axes(
        out,
        maximum,
        4,
        if quality {
            "Density per Q-score unit"
        } else {
            "Density per log10(bp)"
        },
    );
    for i in 0..=4 {
        let value = xmin + (xmax - xmin) * i as f64 / 4.0;
        x_tick(
            out,
            LEFT + (RIGHT - LEFT) * i as f64 / 4.0,
            &if quality {
                decimal(value)
            } else {
                format!("10^{}", compact(value))
            },
        );
    }
    for (i, (run, estimate)) in runs.iter().zip(estimates).enumerate() {
        if let Some(estimate) = estimate {
            let mut path = String::new();
            for (j, point) in estimate.points.iter().enumerate() {
                let x = LEFT + (point.x - xmin) / (xmax - xmin) * (RIGHT - LEFT);
                let y = BOTTOM - point.density / maximum * (BOTTOM - TOP);
                write!(path, " {} {x:.2} {y:.2}", if j == 0 { 'M' } else { 'L' }).unwrap();
            }
            write!(out, "<path class=\"cumulative-line {}\" d=\"{path}\"><title>{}: Gaussian bandwidth {}</title></path>", run_color(i), escape(&run.source.source_id), decimal(estimate.bandwidth)).unwrap();
            if constant {
                let point = &estimate.points[0];
                let x = LEFT + (point.x - xmin) / (xmax - xmin) * (RIGHT - LEFT);
                let y = BOTTOM - point.density / maximum * (BOTTOM - TOP);
                write!(out, "<circle class=\"constant-density-point {}\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"3\"><title>{}: density {} at {}</title></circle>", run_color(i), escape(&run.source.source_id), decimal(point.density), decimal(point.x)).unwrap();
            }
        }
    }
    write!(out, "<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">{}</text></svg></div>", if quality { "Mean read Q-score" } else { "Read length (bp; logarithmic axis)" }).unwrap();
    run_legend(out, runs);
    if constant {
        out.push_str("<p class=\"annotation\">Constant observed domain: points show density evaluated at the single shared coordinate; no continuous curve is available. Only the display axis is expanded.</p>");
    }
    out.push_str("</figure>");
}

fn run_legend(out: &mut String, runs: &[crate::report::RunComparison]) {
    out.push_str("<p class=\"annotation\">");
    for (i, run) in runs.iter().enumerate() {
        write!(
            out,
            "<span class=\"legend {}\">{}</span>",
            run_color(i),
            escape(&run.source.source_id)
        )
        .unwrap();
    }
    out.push_str("</p>");
}

fn comparison_yield_length(out: &mut String, report: &Report, selected: bool, population: &str) {
    let runs = &report.collection.as_ref().unwrap().runs;
    let id = format!("combined-yield-length-{selected}");
    figure(
        out,
        &id,
        &format!("Yield by minimum length by run · {population}"),
    );
    let maximum = runs
        .iter()
        .map(|r| {
            if selected {
                r.qscore_selected.total_yield_bp
            } else {
                r.metrics.total_yield_bp
            }
        })
        .max()
        .unwrap_or(0);
    if maximum == 0 {
        empty_chart(out, "No eligible yield.");
        return;
    }
    let xmax = runs
        .iter()
        .filter_map(|r| r.yield_by_minimum_length.curve.last())
        .map(|p| p.minimum_length_bp)
        .max()
        .unwrap_or(1) as f64;
    let (scale, unit) = bp_scale(maximum);
    let (ymax, ticks) = plot_scale(maximum as f64 / scale, scale == 1.0);
    svg_start(
        out,
        &id,
        "Exact retained yield at bounded queries for each source; absolute yield, not normalized density.",
        294,
    );
    axes(out, ymax, ticks, &format!("Retained yield ({unit})"));
    for i in 0..=4 {
        x_tick(
            out,
            LEFT + (RIGHT - LEFT) * i as f64 / 4.0,
            &compact(xmax / 1000.0 * i as f64 / 4.0),
        );
    }
    for (i, run) in runs.iter().enumerate() {
        let mut path = String::new();
        for (j, point) in run.yield_by_minimum_length.curve.iter().enumerate() {
            let yield_bp = if selected {
                point.qscore_selected_yield_bp
            } else {
                point.all_yield_bp
            };
            let x = LEFT + point.minimum_length_bp as f64 / xmax * (RIGHT - LEFT);
            let y = BOTTOM - yield_bp as f64 / scale / ymax * (BOTTOM - TOP);
            write!(path, " {} {x:.2} {y:.2}", if j == 0 { 'M' } else { 'L' }).unwrap();
        }
        write!(
            out,
            "<path class=\"cumulative-line {}\" d=\"{path}\"><title>{}</title></path>",
            run_color(i),
            escape(&run.source.source_id)
        )
        .unwrap();
    }
    out.push_str("<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Minimum read length (kb; inclusive)</text></svg></div>");
    run_legend(out, runs);
    out.push_str("</figure>");
}

fn section(out: &mut String, id: &str, number: &str, title: &str, note: &str) {
    write!(out, "<section id=\"{id}\" aria-labelledby=\"{id}-heading\"><div class=\"section-heading\"><span class=\"section-number\">{number}</span><div><h2 id=\"{id}-heading\">{title}</h2><p>{note}</p></div></div>").unwrap();
}

fn overview(out: &mut String, report: &Report) {
    let m = &report.metrics;
    if report.input.excluded_control_records > 0 {
        write!(out, "<p class=\"notice\">{} YHR174W control observations excluded from {} input records (MinIONQC-compatible policy).</p>", count(report.input.excluded_control_records), count(report.input.input_records)).unwrap();
    }
    section(
        out,
        "overview",
        "01",
        if report.collection.is_some() {
            "Pooled overview"
        } else {
            "Run overview"
        },
        if report.collection.is_some() {
            "Exact pooled statistics · Independent runs / explicitly identified sources"
        } else {
            "Oxford Nanopore sequencing summary · Single-run analysis"
        },
    );
    if !report.comparison.native_filtering_available {
        out.push_str("<p class=\"notice\">Native filtering unavailable or incomplete: passes_filtering is absent or missing on some reads. Native count fields in JSON count observed TRUE/FALSE only; no reads are inferred to fail. Q-cutoff comparisons remain available.</p>");
    }
    let run_id = if report.collection.is_some() {
        "Pooled runs / sources"
    } else {
        report.run.run_id.as_deref().unwrap_or("Not available")
    };
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
        "Native passed yield",
        report
            .comparison
            .native_filtering_available
            .then(|| m.passed_yield_bp.to_string()),
        if report.comparison.native_filtering_available {
            format!("{} Gb", decimal(m.passed_yield_bp as f64 / 1e9))
        } else {
            "Not available".into()
        },
    );
    let rate = report
        .comparison
        .native_filtering_available
        .then(|| m.pass_rate_percent())
        .flatten();
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
            &compact(end / 3600.0 * i as f64 / 4.0),
        );
    }
    out.push_str("<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Time from run start (h)</text>");
    time_markers(out, report, end);
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
    annotated_bars(
        out,
        "length-counts",
        "Reads by length interval",
        "Basecalled reads (reads)",
        1.0,
        &reads,
        statistic_annotation(report, false, false).as_deref(),
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
    annotated_bars(
        out,
        "length-yield",
        "Yield by length interval",
        &format!("Yield ({unit})"),
        scale,
        &yields,
        statistic_annotation(report, false, false).as_deref(),
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
    if report.collection.is_none() {
        read_time_plot(out, report, false);
        fitted_trend_plot(out, report, false, false);
    }
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
    annotated_bars(
        out,
        "quality-histogram",
        "Read Q-score distribution",
        "Basecalled reads (reads)",
        1.0,
        &histogram,
        statistic_annotation(report, true, false).as_deref(),
    );
    out.push_str("<div class=\"chart-grid\">");
    let m = &report.metrics;
    if report.comparison.native_filtering_available {
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
    } else {
        out.push_str("<p class=\"annotation\">Native pass / fail unavailable: passes_filtering is absent.</p>");
    }
    out.push_str("</div><p class=\"annotation\"><span class=\"legend pass\">Native pass</span><span class=\"legend fail\">Fail</span> Native <code>passes_filtering</code> categories; no automatic run-level QC judgment.</p><details><summary>View exact Q-score histogram</summary><div class=\"table-scroll\"><table><caption>Per-read Q-score intervals, lower-inclusive and upper-exclusive</caption><thead><tr><th class=\"label\" scope=\"col\">Q-score interval</th><th scope=\"col\">Reads</th></tr></thead><tbody>");
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
    if report.collection.is_none() {
        read_time_plot(out, report, true);
        fitted_trend_plot(out, report, true, false);
    }
    length_quality_heatmaps(out, report);
    out.push_str("</section>");
}

fn qscore_comparison(out: &mut String, report: &Report) {
    let comparison = &report.comparison;
    let label = format!("Q ≥ {}", cutoff_value(comparison.qscore_cutoff));
    section(
        out,
        "qcutoff",
        "09",
        "Q-score cutoff comparison",
        "Independent of the basecaller's native passes_filtering classification.",
    );
    write!(out, "<p class=\"annotation\">Selected population: <strong>{}</strong> and positive read length. {} reads · {} bp. Zero-length summary records are excluded even at Q ≥ 0. Native passed results elsewhere retain their original meaning.</p>", escape(&label), count(comparison.qscore_selected.read_count), count(comparison.qscore_selected.total_yield_bp)).unwrap();
    out.push_str("<div class=\"chart-grid\">");
    for (id, title, yield_plot) in [
        (
            "qcutoff-length-counts",
            "Q-cutoff read length distribution",
            false,
        ),
        (
            "qcutoff-length-yield",
            "Q-cutoff yield by length interval",
            true,
        ),
    ] {
        let rows: Vec<_> = comparison
            .length_histogram
            .iter()
            .map(|b| {
                (
                    length_interval(b.lower_inclusive_bp, b.upper_exclusive_bp),
                    if yield_plot { b.yield_bp } else { b.read_count },
                    "qselected",
                )
            })
            .collect();
        let (scale, unit) = if yield_plot {
            bp_scale(rows.iter().map(|r| r.1).max().unwrap_or(0))
        } else {
            (1.0, "reads")
        };
        annotated_bars(
            out,
            id,
            &format!("{title} · {label}"),
            &format!(
                "{} ({unit})",
                if yield_plot {
                    "Yield"
                } else {
                    "Basecalled reads"
                }
            ),
            scale,
            &rows,
            statistic_annotation(report, false, true).as_deref(),
        );
    }
    let rows: Vec<_> = comparison
        .quality_histogram
        .iter()
        .map(|b| {
            (
                quality_interval(b.lower_inclusive, b.upper_exclusive),
                b.read_count,
                "qselected",
            )
        })
        .collect();
    annotated_bars(
        out,
        "qcutoff-quality",
        &format!("Read Q-score distribution · {label}"),
        "Basecalled reads (reads)",
        1.0,
        &rows,
        statistic_annotation(report, true, true).as_deref(),
    );
    out.push_str("</div>");
    if report.collection.is_none() {
        for (id, title, axis, cumulative) in [
            (
                "qcutoff-yield-time",
                "Cumulative yield · All vs Q-cutoff",
                "Cumulative yield (Gb)",
                true,
            ),
            (
                "qcutoff-reads-time",
                "Read count over time · All vs Q-cutoff",
                "Basecalled reads per 15 min",
                false,
            ),
        ] {
            let all = TimeSeries {
                label: "All basecalled reads",
                color: "primary",
                bin_offset_seconds: if cumulative { 900 } else { 450 },
                points: report
                    .time_bins
                    .iter()
                    .map(|b| {
                        (
                            b.start_seconds,
                            Some(if cumulative {
                                b.cumulative_yield_bp as f64 / 1e9
                            } else {
                                b.basecalled_reads as f64
                            }),
                        )
                    })
                    .collect(),
            };
            let selected = TimeSeries {
                label: &label,
                color: "qselected",
                bin_offset_seconds: if cumulative { 900 } else { 450 },
                points: comparison
                    .time_bins
                    .iter()
                    .map(|b| {
                        (
                            b.start_seconds,
                            Some(if cumulative {
                                b.cumulative_yield_bp as f64 / 1e9
                            } else {
                                b.basecalled_reads as f64
                            }),
                        )
                    })
                    .collect(),
            };
            time_series_plot(out, report, id, title, axis, &[all, selected], !cumulative);
        }
        out.push_str("<p class=\"annotation\">Read-start-attributed, unsmoothed 15-minute bins; means are shown with the length and quality sections. Cumulative points include each entire interval and do not reconstruct within-bin throughput. Exact cutoff histograms and time-bin values are retained in JSON.</p>");
    }
    yield_length_plot(out, report, &label);
    out.push_str("</section>");
}

fn yield_length_plot(out: &mut String, report: &Report, label: &str) {
    let analysis = &report.comparison.yield_by_minimum_length;
    figure(out, "yield-minimum-length", "Yield by minimum read length");
    let maximum = report.metrics.total_yield_bp;
    if maximum == 0 {
        empty_chart(out, "No basecalled yield available.");
    } else {
        let (scale, unit) = bp_scale(maximum);
        let (ymax, ticks) = plot_scale(maximum as f64 / scale, scale == 1.0);
        let xmax = analysis.curve.last().map_or(1, |p| p.minimum_length_bp) as f64;
        svg_start(
            out,
            "yield-minimum-length",
            "Exact retained bases at sampled inclusive minimum lengths; no histogram reconstruction.",
            294,
        );
        axes(out, ymax, ticks, &format!("Retained yield ({unit})"));
        for i in 0..=4 {
            x_tick(
                out,
                LEFT + (RIGHT - LEFT) * i as f64 / 4.0,
                &compact(xmax / 1000.0 * i as f64 / 4.0),
            );
        }
        out.push_str("<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Minimum read length (kb; inclusive)</text>");
        for selected in [false, true] {
            let mut path = String::new();
            for (i, point) in analysis.curve.iter().enumerate() {
                let bases = if selected {
                    point.qscore_selected_yield_bp
                } else {
                    point.all_yield_bp
                };
                let x = LEFT + point.minimum_length_bp as f64 / xmax * (RIGHT - LEFT);
                let y = BOTTOM - bases as f64 / scale / ymax * (BOTTOM - TOP);
                write!(path, " {} {x:.2} {y:.2}", if i == 0 { 'M' } else { 'L' }).unwrap();
                write!(out, "<circle class=\"{}\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"2\"><title>{}: minimum {} bp, {} bp retained</title></circle>", if selected { "qselected" } else { "primary" }, if selected { escape(label) } else { "All basecalled reads".into() }, count(point.minimum_length_bp), count(bases)).unwrap();
            }
            write!(
                out,
                "<path class=\"{}\" d=\"{path}\"/>",
                if selected {
                    "qselected-line"
                } else {
                    "cumulative-line"
                }
            )
            .unwrap();
        }
        write!(out, "</svg></div><p class=\"annotation\"><span class=\"legend all\">All basecalled reads</span><span class=\"legend qselected-key\">{}</span></p></figure>", escape(label)).unwrap();
    }
    write!(out, "<p class=\"annotation\">Every point is calculated from exact lengths. At most 520 deterministic display queries span the full range; connecting segments summarize between queries, not every observed length. All seven requested thresholds remain exact below and in JSON, including those beyond the observed maximum. Upstream labels these thresholds “&gt;”; its code uses ≥.</p><div class=\"table-scroll\"><table><caption>Inclusive length thresholds and specialized ultralong subset</caption><thead><tr><th class=\"label\" scope=\"col\">Selection</th><th scope=\"col\">All reads</th><th scope=\"col\">All yield (bp)</th><th scope=\"col\">{} reads</th><th scope=\"col\">{} yield (bp)</th></tr></thead><tbody>", escape(label), escape(label)).unwrap();
    for t in &analysis.thresholds {
        write!(out, "<tr><th class=\"label\" scope=\"row\">≥ {} bp</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", count(t.minimum_length_bp), count(t.all_read_count), count(t.all_yield_bp), count(t.qscore_selected_read_count), count(t.qscore_selected_yield_bp)).unwrap();
    }
    let a = &analysis.ultralong_all;
    let q = &analysis.ultralong_qscore_selected;
    write!(out, "<tr><th class=\"label\" scope=\"row\">Ultralong prefix</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr></tbody></table></div><p class=\"annotation\">Ultralong is the largest descending-length prefix whose strict-half N50 is ≥100 kb, not the subset of reads ≥100 kb. This dedicated comparison uses MinIONQC's strict half-yield convention; established PoreGauge N50 uses at least half. Eligible full sets and empty sets are handled explicitly.</p>", count(a.read_count), count(a.total_yield_bp), count(q.read_count), count(q.total_yield_bp)).unwrap();
}

fn channel_summary_distributions(out: &mut String, report: &Report) {
    let label = format!("Q ≥ {}", cutoff_value(report.comparison.qscore_cutoff));
    out.push_str("<h3>Channel summary distributions · All summary records vs Q-cutoff</h3><p class=\"annotation\">Each observed channel counts once per population. For upstream comparison, All summary records includes 0 bp lengths in channel counts, means and medians. Q-cutoff uses positive-length reads only. Unobserved channels are absent; channels without selected reads are absent from the selected distributions. Existing basecalled/native metrics retain their denominators. Exact medians and counts are in the channel table and JSON.</p><div class=\"chart-grid\">");
    for distribution in &report.comparison.channel_distributions {
        let (title, axis, scale) = match distribution.metric {
            "total_yield_bp" => ("Total bases per channel", "Yield per channel (Mb)", 1e6),
            "read_count" => (
                "Read count per channel",
                "Summary records / selected reads per channel",
                1.0,
            ),
            "mean_read_length_bp" => (
                "Mean read length per channel",
                "Mean read length (kb)",
                1000.0,
            ),
            _ => (
                "Median read length per channel",
                "Median read length (kb)",
                1000.0,
            ),
        };
        let id = format!("channel-summary-{}", distribution.metric);
        figure(out, &id, title);
        let max = distribution
            .bins
            .iter()
            .map(|b| b.all_summary_channels.max(b.qscore_selected_channels))
            .max()
            .unwrap_or(0);
        if max == 0 {
            empty_chart(out, "No observed channels.");
            continue;
        }
        let (ymax, ticks) = plot_scale(max as f64, true);
        svg_start(
            out,
            &id,
            "Shared 30 equal-width intervals; last interval includes the maximum. Bar height is the number of channels, not reads.",
            294,
        );
        axes(out, ymax, ticks, "Channels (channels)");
        let width = (RIGHT - LEFT) / distribution.bins.len() as f64;
        for (i, bin) in distribution.bins.iter().enumerate() {
            for (selected, value) in [
                (false, bin.all_summary_channels),
                (true, bin.qscore_selected_channels),
            ] {
                let height = value as f64 / ymax * (BOTTOM - TOP);
                let x = LEFT + width * (i as f64 + if selected { 0.5 } else { 0.0 });
                write!(out, "<rect class=\"{}\" x=\"{x:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{height:.2}\"><title>{}: [{}, {}) · {} channels</title></rect>", if selected { "qselected" } else { "primary" }, BOTTOM - height, width * 0.45, if selected { escape(&label) } else { "All summary records".into() }, decimal(bin.lower_inclusive), bin.upper_exclusive.map(decimal).unwrap_or_else(|| "∞".into()), count(value)).unwrap();
            }
        }
        for i in 0..=4 {
            x_tick(
                out,
                LEFT + (RIGHT - LEFT) * i as f64 / 4.0,
                &compact(distribution.axis_maximum / scale * i as f64 / 4.0),
            );
        }
        write!(out, "<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">{axis}</text></svg></div><p class=\"annotation\"><span class=\"legend all\">All summary records</span><span class=\"legend qselected-key\">{}</span></p></figure>", escape(&label)).unwrap();
    }
    out.push_str("</div>");
}

fn read_comparison(out: &mut String, report: &Report) {
    let all = &report.metrics;
    let passed = &report.diagnostics.passed_read_statistics;
    let selected = &report.comparison.qscore_selected;
    write!(out, "<p class=\"annotation\">Independent Q-cutoff: Q ≥ {}. Native passed is a separate population.</p>", cutoff_value(report.comparison.qscore_cutoff)).unwrap();
    out.push_str("<div class=\"table-scroll comparison-table\"><table id=\"read-statistics\"><caption>All basecalled, native passed and Q-cutoff selected reads · zero-length records excluded</caption><thead><tr><th class=\"label\" scope=\"col\">Metric</th><th scope=\"col\">All basecalled reads</th><th class=\"passed-heading\" scope=\"col\">Native passed reads</th><th scope=\"col\">Q-cutoff selected reads</th></tr></thead><tbody>");
    for (label, key, a, p) in [
        (
            "Read count",
            "read_count",
            Some(all.basecalled_reads),
            report
                .comparison
                .native_filtering_available
                .then_some(passed.read_count),
        ),
        (
            "Total yield (bp)",
            "total_yield_bp",
            Some(all.total_yield_bp),
            report
                .comparison
                .native_filtering_available
                .then_some(passed.total_yield_bp),
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
            match key {
                "read_count" => Some(selected.read_count),
                "total_yield_bp" => Some(selected.total_yield_bp),
                "n50_bp" => selected.n50_bp,
                "min_read_length_bp" => selected.min_read_length_bp,
                _ => selected.max_read_length_bp,
            }
            .map(|v| (Some(v.to_string()), count(v)))
            .unwrap_or((None, "Not available".into())),
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
            {
                let value = match key {
                    "mean_read_length_bp" => selected.mean_read_length_bp,
                    "median_read_length_bp" => selected.median_read_length_bp,
                    "mean_qscore" => selected.mean_qscore,
                    _ => selected.median_qscore,
                };
                (value.map(|v| v.to_string()), optional(value, 1.0, ""))
            },
        );
    }
    out.push_str("</tbody></table></div><p class=\"annotation\">All three basecalled populations use exact sorted middle values and N50, not histogram estimates. Native passed reads use only <code>passes_filtering=TRUE</code>, with no Q-score cutoff. Mean Q-score is arithmetic per read.</p>");
}

fn comparison_row(
    out: &mut String,
    label: &str,
    key: &str,
    all: (Option<String>, String),
    passed: (Option<String>, String),
    selected: (Option<String>, String),
) {
    write!(
        out,
        "<tr><th class=\"label\" scope=\"row\">{}</th>",
        escape(label)
    )
    .unwrap();
    for (subset, (raw, display)) in [
        ("all", all),
        ("passed", passed),
        ("qscore_selected", selected),
    ] {
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
    let selected_label = format!("Q ≥ {}", cutoff_value(report.comparison.qscore_cutoff));
    let mut series = [
        TimeSeries {
            label: "All basecalled reads",
            color: "primary",
            bin_offset_seconds: 450,
            points: Vec::new(),
        },
        TimeSeries {
            label: if report.comparison.native_filtering_available {
                "Native passed reads"
            } else {
                "Native passed · unavailable"
            },
            color: "passed",
            bin_offset_seconds: 450,
            points: Vec::new(),
        },
        TimeSeries {
            label: &selected_label,
            color: "qselected",
            bin_offset_seconds: 450,
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
    for bin in &report.comparison.time_bins {
        series[2].points.push((
            bin.start_seconds,
            if quality {
                bin.mean_qscore
            } else {
                bin.mean_read_length_bp.map(|v| v / 1000.0)
            },
        ));
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
            "Read length vs Q-score · Native passed reads",
            &h.passed_read_counts,
        ),
    ] {
        if id == "length-quality-passed" && !report.comparison.native_filtering_available {
            figure(out, id, title);
            empty_chart(
                out,
                "Native filtering unavailable or incomplete: passes_filtering is absent or missing on some reads.",
            );
        } else {
            length_quality_map(out, h, id, title, counts, maximum);
        }
    }
    out.push_str("</div><details><summary>View exact length × Q-score cell counts</summary><div class=\"table-scroll\"><table><caption>Nonzero all-read cells; omitted cells contain zero reads in both subsets. Intervals are lower-inclusive and upper-exclusive; final intervals are unbounded.</caption><thead><tr><th class=\"label\" scope=\"col\">Length interval (bp)</th><th class=\"label\" scope=\"col\">Q-score interval</th><th scope=\"col\">All reads</th><th scope=\"col\">Passed reads</th></tr></thead><tbody>");
    let mut populated = false;
    for (q, row) in h.all_read_counts.iter().enumerate() {
        for (l, &reads) in row.iter().enumerate() {
            if reads == 0 {
                continue;
            }
            populated = true;
            write!(out, "<tr><th class=\"label\" scope=\"row\">{}</th><td class=\"label\">{}</td><td>{}</td><td>{}</td></tr>", length_interval(h.length_lower_bounds_bp[l], h.length_lower_bounds_bp.get(l+1).copied()), quality_interval(h.qscore_lower_bounds[q], h.qscore_lower_bounds.get(q+1).copied()), count(reads), native_count(report, h.passed_read_counts[q][l])).unwrap();
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
    minion_read_plots(out, report);
    channel_summary_distributions(out, report);
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
    out.push_str("<p class=\"annotation\">Each eligible channel counts once in its mean-length interval; channels without basecalled reads are excluded. This is a distribution of channel means, not individual read lengths. Exact all/passed channel means are in the table; exact per-channel basecalled, summary-record and Q-cutoff medians are included below.</p>");
    out.push_str("<p class=\"print-only notice\">Detailed channel and channel × mux tables are omitted from print. Complete exact values remain available in report.json and the offline HTML report.</p>");
    out.push_str(r##"<div id="channel-controls" class="table-controls" hidden><label>Find channel <input id="channel-search" type="search" placeholder="Channel ID" autocomplete="off"></label><label>Sort by <select id="channel-sort"><option value="channel">Channel ID ↑</option><option value="yield">Yield ↓</option><option value="reads">Basecalled reads ↓</option><option value="qscore">Mean Q-score ↓</option><option value="meanlength">Mean length ↓</option><option value="passedlength">Mean native passed length ↓</option><option value="medianlength">Median basecalled length ↓</option><option value="qreads">Q-cutoff reads ↓</option><option value="qyield">Q-cutoff yield ↓</option><option value="qmedian">Q-cutoff median length ↓</option></select></label><label>Rows <select id="channel-page-size"><option>25</option><option>50</option><option>100</option></select></label></div>
<div class="table-scroll channel-table detail-table"><table id="channel-table"><caption>All observed channels · exact counts and yield; default order is channel ID</caption><thead><tr><th scope="col">Channel</th><th scope="col">Records</th><th scope="col">Basecalled reads</th><th scope="col">Yield (bp)</th><th scope="col">Native passed reads</th><th scope="col">Native passed yield (bp)</th><th scope="col">Mean Q-score</th><th scope="col">Mean length (bp)</th><th scope="col">Mean native passed length (bp)</th><th scope="col">Median basecalled length (bp)</th><th scope="col">Mean summary length (bp)</th><th scope="col">Median summary length (bp)</th><th scope="col">Q-cutoff reads</th><th scope="col">Q-cutoff yield (bp)</th><th scope="col">Q-cutoff mean length (bp)</th><th scope="col">Q-cutoff median length (bp)</th></tr></thead><tbody>"##);
    for (c, comparison) in report.channels.iter().zip(&report.comparison.channels) {
        write!(out, "<tr data-channel=\"{}\" data-yield=\"{}\" data-reads=\"{}\" data-qscore=\"{}\" data-meanlength=\"{}\" data-passedlength=\"{}\" data-medianlength=\"{}\" data-qreads=\"{}\" data-qyield=\"{}\" data-qmedian=\"{}\"><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>",
            c.channel, c.total_yield_bp, c.basecalled_reads, c.mean_qscore.map(|v| v.to_string()).unwrap_or_default(), c.mean_read_length_bp.map(|v| v.to_string()).unwrap_or_default(), c.mean_passed_read_length_bp.map(|v| v.to_string()).unwrap_or_default(), comparison.all_basecalled_reads.median_read_length_bp.map(|v| v.to_string()).unwrap_or_default(), comparison.qscore_selected_reads.read_count, comparison.qscore_selected_reads.total_yield_bp, comparison.qscore_selected_reads.median_read_length_bp.map(|v| v.to_string()).unwrap_or_default(), c.channel, count(c.total_records), count(c.basecalled_reads), count(c.total_yield_bp), native_count(report, c.passed_reads), native_count(report, c.passed_yield_bp), optional(c.mean_qscore, 1.0, ""), optional(c.mean_read_length_bp, 1.0, ""), optional(c.mean_passed_read_length_bp, 1.0, "")).unwrap();
        for value in [
            comparison.all_basecalled_reads.median_read_length_bp,
            comparison.all_summary_records.mean_read_length_bp,
            comparison.all_summary_records.median_read_length_bp,
        ] {
            write!(
                out,
                "<td data-value=\"{}\">{}</td>",
                value.map(|v| v.to_string()).unwrap_or_default(),
                optional(value, 1.0, "")
            )
            .unwrap();
        }
        let q = &comparison.qscore_selected_reads;
        write!(
            out,
            "<td>{}</td><td>{}</td><td data-value=\"{}\">{}</td><td data-value=\"{}\">{}</td></tr>",
            count(q.read_count),
            count(q.total_yield_bp),
            q.mean_read_length_bp
                .map(|v| v.to_string())
                .unwrap_or_default(),
            optional(q.mean_read_length_bp, 1.0, ""),
            q.median_read_length_bp
                .map(|v| v.to_string())
                .unwrap_or_default(),
            optional(q.median_read_length_bp, 1.0, "")
        )
        .unwrap();
    }
    if report.channels.is_empty() {
        empty_row(out, 16, "No observed channels.");
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
    if report.comparison.native_filtering_available {
        bars(
            out,
            "mux-passed-yield",
            "Passed yield by mux · native filtering",
            &format!("Yield ({unit})"),
            scale,
            &passed,
        );
    } else {
        out.push_str(
            "<p class=\"annotation\">Native mux yield unavailable: passes_filtering is absent.</p>",
        );
    }
    out.push_str("</div><p class=\"annotation\">Mux identifiers are used as observed, without assuming four values. Read counts and yields do not measure pore occupancy. The channel × mux breakdown is available in the channel section.</p><div class=\"table-scroll\"><table><caption>Exact mux totals</caption><thead><tr><th scope=\"col\">Mux</th><th scope=\"col\">Records</th><th scope=\"col\">Basecalled reads</th><th scope=\"col\">Yield (bp)</th><th scope=\"col\">Passed yield (bp)</th></tr></thead><tbody>");
    for m in &report.muxes {
        write!(
            out,
            "<tr><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            m.mux,
            count(m.total_records),
            count(m.basecalled_reads),
            count(m.total_yield_bp),
            native_count(report, m.passed_yield_bp)
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
        out.push_str("<p class=\"annotation\">Physical channel maps are disabled. Explicitly select a compatible --flowcell-layout promethion-3000 or minion-r9.5.</p>");
        return;
    };
    let minion = matches!(layout.name, crate::layout::FlowcellLayout::MinionR95);
    write!(out, "<h3>{} spatial yield</h3><p class=\"annotation\">Explicit {} layout: {} rows × {} columns, zero-based. {} Unobserved channels do not establish dead or inactive pores.</p>", if minion { "MinION R9.5" } else { "PromethION" }, layout.name.name(), layout.rows, layout.columns, if minion { "Exact MinIONQC p1–p8 / q1–q8 mapping." } else { "Twelve consecutive 25 × 10 blocks from the EPI2ME reference." }).unwrap();
    let channels: std::collections::BTreeMap<_, _> =
        report.channels.iter().map(|c| (c.channel, c)).collect();
    let selected: std::collections::BTreeMap<_, _> = report
        .comparison
        .channels
        .iter()
        .map(|c| (c.channel, &c.qscore_selected_reads))
        .collect();
    let cell = if minion { 16 } else { 10 };
    let bottom = 28 + layout.rows * cell;
    let width = if minion { 760 } else { 1280 };
    let height = bottom + 115;
    for population in 0..3 {
        let (id, title, hue) = match population {
            0 => (
                "spatial-total-yield",
                "Total basecalled yield per physical channel".to_owned(),
                228,
            ),
            1 => (
                "spatial-passed-yield",
                "Passed yield per physical channel · native filtering".into(),
                164,
            ),
            _ => (
                "spatial-qcutoff-yield",
                format!(
                    "Yield per physical channel · Q ≥ {}",
                    cutoff_value(report.comparison.qscore_cutoff)
                ),
                260,
            ),
        };
        figure(out, id, &title);
        if population == 1 && !report.comparison.native_filtering_available {
            empty_chart(out, "Native filtering unavailable or incomplete.");
            continue;
        }
        let value = |channel| {
            channels.get(&channel).map(|c| match population {
                0 => c.total_yield_bp,
                1 => c.passed_yield_bp,
                _ => selected.get(&channel).map_or(0, |c| c.total_yield_bp),
            })
        };
        let maximum = layout
            .channel_coordinates
            .iter()
            .filter_map(|c| value(c.channel))
            .max()
            .unwrap_or(0);
        let (scale, unit) = bp_scale(maximum);
        write!(out, "<p class=\"scroll-hint\">Scroll map horizontally for physical coordinates and values.</p><div class=\"plot-scroll\" tabindex=\"0\" role=\"group\" aria-label=\"Scrollable physical channel heatmap\"><svg class=\"heatmap\" viewBox=\"0 0 {width} {height}\" role=\"group\" aria-labelledby=\"{id}-caption\"><defs><pattern id=\"{id}-unobserved\" width=\"4\" height=\"4\" patternUnits=\"userSpaceOnUse\"><rect width=\"4\" height=\"4\" fill=\"#f1f3f6\"/><path d=\"M0 0L4 4\" stroke=\"#cbd2df\"/></pattern><linearGradient id=\"{id}-scale\"><stop stop-color=\"hsl({hue} 45% 96%)\"/><stop offset=\"1\" stop-color=\"hsl({hue} 45% 42%)\"/></linearGradient></defs>").unwrap();
        for coordinate in &layout.channel_coordinates {
            let v = value(coordinate.channel);
            let fill = v.map_or_else(
                || format!("url(#{id}-unobserved)"),
                |v| {
                    format!(
                        "hsl({hue} 45% {:.2}%)",
                        96.0 - if maximum == 0 {
                            0.0
                        } else {
                            v as f64 / maximum as f64 * 54.0
                        }
                    )
                },
            );
            let description = match v {
                Some(v) => format!(
                    "Channel {} · row {}, column {}: {} bp {}; {} summary records; {} basecalled reads",
                    coordinate.channel,
                    coordinate.row,
                    coordinate.column,
                    count(v),
                    if population == 1 {
                        "passed yield"
                    } else if population == 2 {
                        "Q-cutoff yield"
                    } else {
                        "total yield"
                    },
                    count(channels[&coordinate.channel].total_records),
                    count(channels[&coordinate.channel].basecalled_reads)
                ),
                None => format!(
                    "Channel {} · row {}, column {}: not observed; yield unavailable",
                    coordinate.channel, coordinate.row, coordinate.column
                ),
            };
            write!(out, "<rect class=\"heatmap-cell\" data-channel=\"{}\" data-yield-bp=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\" role=\"img\" aria-labelledby=\"{id}-c{}\"><title id=\"{id}-c{}\">{}</title></rect>", coordinate.channel, v.map(|v| v.to_string()).unwrap_or_default(), 48+coordinate.column*cell, 28+coordinate.row*cell, cell-1, cell-1, coordinate.channel, coordinate.channel, escape(&description)).unwrap();
        }
        let row_ticks: std::collections::BTreeSet<_> = (0..layout.rows)
            .step_by(5)
            .chain([layout.rows - 1])
            .collect();
        for row in row_ticks {
            write!(
                out,
                "<text class=\"tick\" x=\"38\" y=\"{}\" text-anchor=\"end\">{row}</text>",
                36 + row * cell
            )
            .unwrap();
        }
        let column_ticks: std::collections::BTreeSet<_> = (0..layout.columns)
            .step_by(if minion { 4 } else { 20 })
            .chain([layout.columns - 1])
            .collect();
        for column in column_ticks {
            write!(
                out,
                "<text class=\"tick\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{column}</text>",
                52 + column * cell,
                bottom + 18
            )
            .unwrap();
        }
        write!(out, "<text class=\"axis-label\" x=\"48\" y=\"17\">Row</text><text class=\"axis-label\" x=\"{}\" y=\"{}\" text-anchor=\"end\">Column</text><rect x=\"48\" y=\"{}\" width=\"240\" height=\"10\" fill=\"url(#{id}-scale)\"/><text class=\"tick\" x=\"48\" y=\"{}\">0 {unit}</text><text class=\"tick\" x=\"288\" y=\"{}\" text-anchor=\"end\">{} {unit}</text><rect x=\"48\" y=\"{}\" width=\"14\" height=\"14\" fill=\"url(#{id}-unobserved)\"/><text class=\"tick\" x=\"70\" y=\"{}\">Not observed · unavailable yield</text><rect x=\"360\" y=\"{}\" width=\"14\" height=\"14\" fill=\"hsl({hue} 45% 96%)\" stroke=\"#aab4c8\"/><text class=\"tick\" x=\"382\" y=\"{}\">Observed · zero yield</text></svg></div></figure>", 48+layout.columns*cell, bottom+39, bottom+39, bottom+68, bottom+68, decimal(maximum as f64 / scale), bottom+82, bottom+94, bottom+82, bottom+94).unwrap();
    }
}

fn minion_read_plots(out: &mut String, report: &Report) {
    let Some(sample) = &report.diagnostics.minion_sample else {
        return;
    };
    let points: Vec<_> = sample.channels.iter().flat_map(|c| &c.points).collect();
    write!(out, "<h3>MinION read-level diagnostics</h3><p class=\"annotation\">Deterministic channel-stratified reservoir sample: at most {} reads per channel (8,192 total), {} plotted from {} basecalled reads. Points are observed reads, not reconstructed histogram observations. Sampling is not proportional to channel yield; density and point counts are not exact full-run measurements. No identifiers are embedded.</p>", sample.limit_per_channel, count(points.len() as u64), count(sample.eligible_reads)).unwrap();
    figure(
        out,
        "minion-length-quality",
        "MinION read length vs Q-score · sampled basecalled reads",
    );
    if points.is_empty() {
        empty_chart(out, "No sampled basecalled reads.");
        return;
    }
    let max_length = report.metrics.max_read_length_bp.unwrap_or(1) as f64;
    let log_max = max_length.log10().max(1.0);
    let qmin = report.quality.min_qscore.unwrap_or(0.0);
    let qmax = report.quality.max_qscore.unwrap_or(0.0);
    let max_events = points
        .iter()
        .filter_map(|p| p.events_per_base)
        .reduce(f64::max)
        .unwrap_or(0.0);
    svg_start(
        out,
        "minion-length-quality",
        "Observed sampled reads; X is log10 read length, Y is per-read Q-score, color is log10(1+events/base). Gray indicates missing events.",
        324,
    );
    for i in 0..=4 {
        let fraction = i as f64 / 4.0;
        let q = qmin * (1.0 - fraction) + qmax * fraction;
        let y = BOTTOM - (BOTTOM - TOP) * fraction;
        write!(out, "<line class=\"grid-line\" x1=\"{LEFT}\" x2=\"{RIGHT}\" y1=\"{y}\" y2=\"{y}\"/><text class=\"tick\" x=\"68\" y=\"{}\" text-anchor=\"end\">{}</text>", y+4.0, decimal(q)).unwrap();
        x_tick(
            out,
            LEFT + (RIGHT - LEFT) * fraction,
            &format!("10^{}", compact(log_max * fraction)),
        );
    }
    out.push_str("<text class=\"axis-label\" x=\"80\" y=\"17\">Mean read Q-score</text><text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Read length (bp; logarithmic axis)</text>");
    for point in &points {
        let x = LEFT + (point.length_bp as f64).log10() / log_max * (RIGHT - LEFT);
        let y = BOTTOM - plot_fraction(point.qscore, qmin, qmax) * (BOTTOM - TOP);
        let color = point.events_per_base.map_or_else(
            || "#9aa6b6".into(),
            |v| {
                format!(
                    "hsl(260 38% {:.2}%)",
                    90.0 - if max_events == 0.0 {
                        0.0
                    } else {
                        (1.0 + v).log10() / (1.0 + max_events).log10() * 55.0
                    }
                )
            },
        );
        write!(out, "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"2\" fill=\"{color}\"><title>{} bp; Q {}; events/base {}</title></circle>", count(point.length_bp), cutoff_value(point.qscore), optional(point.events_per_base, 1.0, "")).unwrap();
    }
    write!(out, "<text class=\"tick\" x=\"80\" y=\"313\">Events/base: light 0 → dark {} (log10(1+ratio)); gray = unavailable</text></svg></div><p class=\"annotation\">Events/base palette: light 0 → dark {} (log10(1+ratio)); gray = unavailable. Template event counts are never used for 2D reads.</p></figure>", decimal(max_events), decimal(max_events)).unwrap();
    let layout = report.diagnostics.physical_layout.as_ref().unwrap();
    figure(
        out,
        "minion-flowcell-overview",
        "MinION physical flowcell overview · sampled reads",
    );
    let end = report.time_bins.last().map_or(900, |b| b.end_seconds) as f64;
    let sampled: std::collections::BTreeMap<_, _> =
        sample.channels.iter().map(|c| (c.channel, c)).collect();
    write!(out, "<p class=\"annotation\">Each physical panel: X = 0–{} h from run start; Y = 1–{} bp on a logarithmic axis. Indigo light → dark represents Q {}–{}. Channel labels identify panels; blank panels mean no sampled basecalled reads, not dead pores.</p>", decimal(end/3600.0), count(max_length as u64), cutoff_value(qmin), cutoff_value(qmax)).unwrap();
    out.push_str("<p class=\"scroll-hint\">Scroll the physical overview horizontally to read all channel panels.</p><div class=\"plot-scroll\" tabindex=\"0\" role=\"group\" aria-label=\"Scrollable MinION flowcell overview\"><svg class=\"plot minion-overview\" viewBox=\"0 0 1320 2240\" role=\"img\" aria-labelledby=\"minion-flowcell-overview-caption\"><title>Exact R9.5 physical panels; sampled observations on shared time and logarithmic length scales, colored by per-read Q-score.</title><g>");
    for c in &layout.channel_coordinates {
        let x0 = 55 + c.column * 76;
        let y0 = 30 + c.row * 66;
        write!(out, "<g data-channel=\"{}\" data-row=\"{}\" data-column=\"{}\"><rect x=\"{x0}\" y=\"{y0}\" width=\"70\" height=\"59\" fill=\"#f0f2f7\" stroke=\"#d7ddeb\"/><text x=\"{}\" y=\"{}\" font-size=\"10\">ch {}</text>", c.channel,c.row,c.column,x0+2,y0+11,c.channel).unwrap();
        if let Some(channel) = sampled.get(&c.channel) {
            for point in &channel.points {
                let x = x0 as f64 + 3.0 + point.start_seconds / end * 64.0;
                let y = y0 as f64 + 55.0 - (point.length_bp as f64).log10() / log_max * 40.0;
                let light = 90.0 - plot_fraction(point.qscore, qmin, qmax) * 55.0;
                write!(out, "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"2\" fill=\"hsl(228 45% {light:.2}%)\"><title>Channel {}: {} s; {} bp; Q {}</title></circle>",c.channel,cutoff_value(point.start_seconds),count(point.length_bp),cutoff_value(point.qscore)).unwrap();
            }
        }
        out.push_str("</g>");
    }
    write!(out, "</g><text class=\"axis-label\" x=\"80\" y=\"2158\">Each channel: X = 0–{} h from run start; Y = 1–{} bp (logarithmic)</text><text class=\"tick\" x=\"80\" y=\"2182\">Indigo light → dark: Q {}–{}; at most 16 sampled reads/channel; blank = no sampled reads</text><text class=\"tick\" x=\"80\" y=\"2206\">Physical columns 0–15; rows 0–31 (top to bottom)</text></svg></div></figure>", decimal(end/3600.0), count(max_length.max(10.0) as u64), cutoff_value(qmin), cutoff_value(qmax)).unwrap();
}

fn plot_fraction(value: f64, minimum: f64, maximum: f64) -> f64 {
    if maximum == minimum {
        return 0.5;
    }
    let scale = minimum.abs().max(maximum.abs()).max(1.0);
    ((value / scale - minimum / scale) / (maximum / scale - minimum / scale)).clamp(0.0, 1.0)
}

fn channel_mux_table(out: &mut String, report: &Report) {
    out.push_str(r##"<details><summary>View channel × mux yield breakdown</summary><p class="annotation">Observed channel/mux pairs only. Bars compare total yield across pairs. Counts and yield are summary-derived, not pore occupancy.</p>
<div id="channel-mux-controls" class="table-controls" hidden><label>Find channel <input id="channel-mux-search" type="search" placeholder="Channel ID" autocomplete="off"></label><label>Sort by <select id="channel-mux-sort"><option value="channel">Channel ID ↑</option><option value="mux">Mux ID ↑</option><option value="yield">Yield ↓</option><option value="reads">Basecalled reads ↓</option></select></label><label>Rows <select id="channel-mux-page-size"><option>25</option><option>50</option><option>100</option></select></label></div>
<div class="table-scroll detail-table"><table id="channel-mux-table"><caption>Channel × mux · exact counts and bp; default order is channel then mux</caption><thead><tr><th scope="col">Channel</th><th scope="col">Mux</th><th scope="col">Records</th><th scope="col">Basecalled reads</th><th scope="col">Zero-length records</th><th scope="col">Yield (bp)</th><th scope="col">Native passed reads</th><th scope="col">Native passed yield (bp)</th></tr></thead><tbody>"##);
    let maximum = report
        .diagnostics
        .channel_mux
        .iter()
        .map(|m| m.total_yield_bp)
        .max()
        .unwrap_or(1)
        .max(1);
    for m in &report.diagnostics.channel_mux {
        write!(out, "<tr data-channel=\"{}\" data-mux=\"{}\" data-yield=\"{}\" data-reads=\"{}\"><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"bar-cell\"><span class=\"record-count\">{}</span><span class=\"table-bar\" style=\"--bar-width:{:.2}%\" aria-hidden=\"true\"></span></td><td>{}</td><td>{}</td></tr>", m.channel, m.mux, m.total_yield_bp, m.basecalled_reads, m.channel, m.mux, count(m.total_records), count(m.basecalled_reads), count(m.zero_length_records), count(m.total_yield_bp), bar_percent(m.total_yield_bp, maximum), native_count(report, m.passed_reads), native_count(report, m.passed_yield_bp)).unwrap();
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
            bin_offset_seconds: 450,
            points,
        }],
        !speed,
    );
}

struct TimeSeries<'a> {
    label: &'a str,
    color: &'static str,
    bin_offset_seconds: u64,
    points: Vec<(u64, Option<f64>)>,
}

// Shared only by start-time diagnostics: common axes, bin midpoints and gaps.
fn time_series_plot(
    out: &mut String,
    report: &Report,
    id: &str,
    title: &str,
    axis: &str,
    series: &[TimeSeries<'_>],
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
    let minimum = series
        .iter()
        .flat_map(|s| s.points.iter().filter_map(|(_, v)| *v))
        .reduce(f64::min)
        .unwrap_or(0.0)
        .min(0.0);
    let maximum = if maximum == 0.0 && minimum == 0.0 {
        1.0
    } else {
        maximum.max(0.0)
    };
    // Finite extreme ratios remain valid data; avoid overflow in rounded axes.
    let (ymax, ticks) = if minimum < 0.0 || !(1e-200..1e300).contains(&maximum) {
        (maximum, 4)
    } else {
        plot_scale(maximum, integer_ticks)
    };
    let end = series
        .iter()
        .flat_map(|s| s.points.iter().map(|(start, _)| *start))
        .max()
        .map_or(900, |start| start + 900) as f64;
    svg_start(out, id, title, 294);
    axes_range(out, minimum, ymax, ticks, axis);
    for i in 0..=4 {
        x_tick(
            out,
            LEFT + (RIGHT - LEFT) * i as f64 / 4.0,
            &compact(end / 3600.0 * i as f64 / 4.0),
        );
    }
    out.push_str("<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Time from run start (h)</text>");
    time_markers(out, report, end);
    for s in series {
        let mut path = String::new();
        let mut previous = None;
        for &(start, value) in &s.points {
            let Some(value) = value else {
                previous = None;
                continue;
            };
            let x = LEFT + (start as f64 + s.bin_offset_seconds as f64) / end * (RIGHT - LEFT);
            let y = BOTTOM - plot_fraction(value, minimum, ymax) * (BOTTOM - TOP);
            let command = if previous.is_some_and(|last| start - last == 900) {
                'L'
            } else {
                'M'
            };
            write!(path, " {command} {x:.2} {y:.2}").unwrap();
            write!(out, "<circle class=\"{}\" cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"3\" data-value=\"{value}\"><title>[{}, {}) seconds · {}: {} ({})</title></circle>", s.color, start, start + 900, escape(s.label), decimal(value), escape(axis)).unwrap();
            previous = Some(start);
        }
        let class = match s.color {
            "passed" => "passed-line",
            "qselected" => "qselected-line",
            _ => "cumulative-line",
        };
        if s.color.starts_with("run") {
            write!(out, "<path class=\"{class} {}\" d=\"{path}\"/>", s.color).unwrap();
        } else {
            write!(out, "<path class=\"{class}\" d=\"{path}\"/>").unwrap();
        }
    }
    out.push_str("</svg></div>");
    if series.len() > 1 {
        out.push_str("<p class=\"annotation\">");
        for s in series {
            let key = match s.color {
                "passed" => "pass",
                "qselected" => "qselected-key",
                color if color.starts_with("run") => color,
                _ => "all",
            };
            write!(
                out,
                "<span class=\"legend {key}\">{}</span>",
                escape(s.label)
            )
            .unwrap();
        }
        out.push_str("</p>");
    }
    out.push_str("</figure>");
}

// Configured reference lines share the plotted relative-time coordinate system.
fn fitted_trend_plot(out: &mut String, report: &Report, quality: bool, selected: bool) {
    let metric = if quality { "qscore" } else { "length_bp" };
    let selected_label = format!(
        "Q ≥ {} summary records",
        cutoff_value(report.comparison.qscore_cutoff)
    );
    let mut curves = Vec::new();
    if let Some(collection) = &report.collection {
        for (i, run) in collection.runs.iter().enumerate() {
            if let Some(trends) = &run.fitted_trends {
                for curve in &trends.combined {
                    if curve.metric == metric && curve.population.starts_with("qscore") == selected
                    {
                        curves.push((run.source.source_id.as_str(), run_color(i), curve));
                    }
                }
            }
        }
    } else if let Some(trends) = &report.fitted_trends {
        for curve in &trends.single {
            if curve.metric == metric {
                curves.push((
                    if curve.population.starts_with("qscore") {
                        selected_label.as_str()
                    } else {
                        "All summary records"
                    },
                    if curve.population.starts_with("qscore") {
                        "qselected"
                    } else {
                        "primary"
                    },
                    curve,
                ));
            }
        }
    }
    if curves.is_empty() {
        return;
    }
    let id = format!("fitted-{quality}-{selected}");
    let population = if report.collection.is_some() {
        if selected {
            format!(
                " · Q ≥ {} summary records",
                cutoff_value(report.comparison.qscore_cutoff)
            )
        } else {
            " · All summary records".into()
        }
    } else {
        String::new()
    };
    figure(
        out,
        &id,
        &format!(
            "Fitted {} from individual summary records{population}",
            if quality { "Q-score" } else { "read length" }
        ),
    );
    let allowed = |v: f64| v >= 0.0 || quality && report.collection.is_some();
    let values: Vec<_> = curves
        .iter()
        .flat_map(|(_, _, c)| &c.points)
        .filter(|p| allowed(p.value))
        .collect();
    if values.is_empty() {
        empty_chart(
            out,
            "Fitted curve unavailable: insufficient distinct times or a singular model. Raw interval charts remain available.",
        );
        return;
    }
    let end = values
        .iter()
        .map(|p| p.hours)
        .reduce(f64::max)
        .unwrap()
        .max(f64::MIN_POSITIVE);
    let scale = if quality { 1.0 } else { 1000.0 };
    let minimum = values
        .iter()
        .map(|p| p.value / scale)
        .reduce(f64::min)
        .unwrap()
        .min(0.0);
    let maximum = values
        .iter()
        .map(|p| p.value / scale)
        .reduce(f64::max)
        .unwrap()
        .max(1.0);
    svg_start(
        out,
        &id,
        "Provisional raw-observation fit; numerical equivalence to R has not been verified. Confidence intervals unavailable.",
        294,
    );
    axes_range(
        out,
        minimum,
        maximum,
        4,
        if quality {
            "Fitted mean Q-score"
        } else {
            "Fitted mean length (kb)"
        },
    );
    for i in 0..=4 {
        x_tick(
            out,
            LEFT + (RIGHT - LEFT) * i as f64 / 4.0,
            &compact(end * i as f64 / 4.0),
        );
    }
    time_markers(out, report, end * 3600.0);
    for (label, color, curve) in &curves {
        let mut path = String::new();
        let mut connected = false;
        for p in &curve.points {
            if !allowed(p.value) {
                connected = false;
                continue;
            }
            write!(
                path,
                " {} {:.2} {:.2}",
                if connected { 'L' } else { 'M' },
                LEFT + p.hours / end * (RIGHT - LEFT),
                BOTTOM - plot_fraction(p.value / scale, minimum, maximum) * (BOTTOM - TOP)
            )
            .unwrap();
            connected = true;
        }
        let class = if *color == "qselected" {
            "qselected-line"
        } else {
            "cumulative-line"
        };
        let run_class = if color.starts_with("run") { *color } else { "" };
        write!(out,"<path class=\"{class} {run_class}\" d=\"{path}\"><title>{}: {}; {} observations; exact knots: {}</title></path>",escape(label),curve.method,curve.observations,curve.exact_knots).unwrap();
    }
    out.push_str("<text class=\"axis-label\" x=\"397\" y=\"286\" text-anchor=\"middle\">Time from run start (h)</text></svg></div><p class=\"annotation\">");
    for (label, color, _) in curves {
        write!(
            out,
            "<span class=\"legend {}\">{}</span>",
            if color == "primary" {
                "all"
            } else if color == "qselected" {
                "qselected-key"
            } else {
                color
            },
            escape(label)
        )
        .unwrap();
    }
    out.push_str("</p><p class=\"annotation\">Provisional MinIONQC-style fit of individual records, including zero lengths. All/Q populations are distinct from native passing. LOESS uses degree 2, span 0.75 and median-cell Hermite interpolation (cell 0.2); GAM uses a 10-knot cubic shrinkage spline and REML. Large groups use deterministic bounded unique-time knot selection. Combined method selection uses the largest group across panels. Confidence intervals and R numerical equivalence are not yet verified. Fits may cross missing intervals; the separate observed-bin charts retain gaps.</p></figure>");
}

fn time_markers(out: &mut String, report: &Report, end: f64) {
    for &seconds in &report.presentation.mux_markers_seconds {
        if seconds <= end {
            let x = LEFT + seconds / end * (RIGHT - LEFT);
            write!(out, "<line class=\"mux-marker\" data-mux-reference-seconds=\"{seconds}\" x1=\"{x:.2}\" x2=\"{x:.2}\" y1=\"{TOP}\" y2=\"{BOTTOM}\"><title>Configured mux reference at {} h; not a measured instrument event</title></line>", decimal(seconds / 3600.0)).unwrap();
        }
    }
}

fn statistic_annotation(report: &Report, quality: bool, selected: bool) -> Option<String> {
    if !report.presentation.add_stat {
        return None;
    }
    let subset = &report.comparison.qscore_selected;
    Some(if quality {
        format!(
            "Mean read Q-score: {}",
            optional(
                if selected {
                    subset.mean_qscore
                } else {
                    report.quality.mean_qscore
                },
                1.0,
                ""
            )
        )
    } else {
        let (mean, n50) = if selected {
            (subset.mean_read_length_bp, subset.n50_bp)
        } else {
            (report.metrics.mean_read_length_bp, report.metrics.n50_bp)
        };
        format!(
            "Mean length: {} · N50: {}",
            optional(mean, 1000.0, "kb"),
            optional(n50.map(|v| v as f64), 1000.0, "kb")
        )
    })
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
    axes_range(out, 0.0, ymax, ticks, label);
}

fn axes_range(out: &mut String, ymin: f64, ymax: f64, ticks: usize, label: &str) {
    for i in 0..=ticks {
        let y = BOTTOM - (BOTTOM - TOP) * i as f64 / ticks as f64;
        write!(out, "<line class=\"grid-line\" x1=\"{LEFT}\" x2=\"{RIGHT}\" y1=\"{y:.2}\" y2=\"{y:.2}\"/><text class=\"tick\" x=\"68\" y=\"{:.2}\" text-anchor=\"end\">{}</text>", y + 4.0, compact(ymin * (1.0 - i as f64 / ticks as f64) + ymax * (i as f64 / ticks as f64))).unwrap();
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
    annotated_bars(out, id, title, axis, scale, rows, None);
}

fn annotated_bars(
    out: &mut String,
    id: &str,
    title: &str,
    axis: &str,
    scale: f64,
    rows: &[(String, u64, &str)],
    annotation: Option<&str>,
) {
    figure(out, id, title);
    let max = rows.iter().map(|r| r.1).max().unwrap_or(0);
    if max == 0 {
        empty_chart(out, "No basecalled observations available.");
        return;
    }
    let height = 80 + rows.len() * 34;
    let baseline = height - 50;
    svg_start(
        out,
        id,
        title,
        height + if annotation.is_some() { 30 } else { 0 },
    );
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
    write!(
        out,
        "<text class=\"axis-label\" x=\"405\" y=\"{}\" text-anchor=\"middle\">{}</text>",
        height - 7,
        escape(axis)
    )
    .unwrap();
    if let Some(annotation) = annotation {
        write!(
            out,
            "<text class=\"stat-annotation\" x=\"165\" y=\"{}\">{}</text>",
            height + 20,
            escape(annotation)
        )
        .unwrap();
    }
    out.push_str("</svg></div></figure>");
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

// Cutoff labels must round-trip, unlike presentation-rounded metric means.
fn cutoff_value(value: f64) -> String {
    if value == 0.0 || (1e-4..1e6).contains(&value.abs()) {
        value.to_string()
    } else {
        format!("{value:e}")
    }
}

fn native_count(report: &Report, value: u64) -> String {
    if report.comparison.native_filtering_available {
        count(value)
    } else {
        "Not available".into()
    }
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
    if value.abs() >= 1e9 {
        format!("{}G", decimal(value / 1e9))
    } else if value.abs() >= 1e6 {
        format!("{}M", decimal(value / 1e6))
    } else if value.abs() >= 1e3 {
        format!("{}k", decimal(value / 1e3))
    } else if value != 0.0 && value.abs() < 0.001 {
        format!("{value:.2e}")
    } else if value.abs() < 1.0 {
        format!("{value:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
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

    #[test]
    fn small_density_ticks_fit_axis_margins_without_changing_raw_values() {
        assert_eq!(super::compact(0.881160859), "0.881");
        assert_eq!(super::compact(0.000001234), "1.23e-6");
        assert_eq!(super::compact(-0.881160859), "-0.881");
        assert_eq!(super::compact(0.0), "0");
        assert_eq!(super::decimal(0.881160859), "0.881160859");
    }
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
        let mut report = crate::metrics::Aggregator::default().finish(None).unwrap();
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
