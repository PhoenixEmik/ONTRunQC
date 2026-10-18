//! Validation-only source-file population: upstream discards run_id and treats
//! each parent folder as a flowcell. Reuse the real parser/aggregator without
//! changing production grouping or retaining complete read objects.
#[test]
#[ignore = "optional R differential harness supplies PG_PROBE_INPUTS/OUTPUT"]
fn export_report() {
    use crate::{
        input::{InputSchema, ReadRepresentation, SummaryReader},
        metrics::Aggregator,
    };
    use std::{fs::File, path::PathBuf};
    let paths: Vec<PathBuf> =
        serde_json::from_str(&std::env::var("PG_PROBE_INPUTS").unwrap()).unwrap();
    let cutoff: f64 = std::env::var("PG_PROBE_CUTOFF").unwrap().parse().unwrap();
    let curves = std::env::var("PG_PROBE_CURVES").as_deref() == Ok("1");
    let mut aggregator = Aggregator::new(None, false);
    aggregator.set_qscore_cutoff(cutoff, true);
    aggregator.enable_reference_density(curves);
    let mut trackers = Vec::new();
    for path in &paths {
        let mut tracker = crate::trends::Tracker::default();
        let mut reader = SummaryReader::with_options(
            File::open(path).unwrap(),
            InputSchema::Auto,
            ReadRepresentation::Auto,
        )
        .unwrap();
        reader.allow_multiple_runs();
        aggregator.include_input_capabilities(
            reader.has_native_filtering(),
            reader.has_template_duration(),
        );
        aggregator
            .include_input_format(reader.schema(), reader.representation())
            .unwrap();
        while let Some(record) = reader.next_record().unwrap() {
            aggregator.add(&record).unwrap();
            if curves {
                tracker.add(&record, cutoff).unwrap();
            }
        }
        trackers.push(tracker);
    }
    let domain = aggregator.density_bounds();
    aggregator.set_density_domain(domain);
    let report = aggregator.finish(None).unwrap();
    let mut report = serde_json::to_value(report).unwrap();
    let mut sources = Vec::new();
    let largest = trackers
        .iter()
        .map(crate::trends::Tracker::count)
        .max()
        .unwrap_or(0);
    if curves {
        for (i, (path, tracker)) in paths.iter().zip(trackers).enumerate() {
            let mut plan = tracker.prepare(largest >= 1000).unwrap();
            let mut source = Aggregator::new(None, false);
            source.set_qscore_cutoff(cutoff, true);
            source.enable_reference_density(true);
            source.set_density_domain(domain);
            let mut reader = SummaryReader::with_options(
                File::open(path).unwrap(),
                InputSchema::Auto,
                ReadRepresentation::Auto,
            )
            .unwrap();
            reader.allow_multiple_runs();
            while let Some(record) = reader.next_record().unwrap() {
                plan.add(&record, cutoff);
                source.add(&record).unwrap();
            }
            let audit = if std::env::var_os("PG_GAM_AUDIT").is_some() {
                plan.audit()
            } else {
                serde_json::Value::Null
            };
            sources.push(serde_json::json!({"source_id":format!("source_{:03}",i+1),"gam_audit":audit,"fitted_trends":plan.finish(), "density":source.finish(None).unwrap().minionqc_density}));
        }
    }
    report["reference_sources"] = serde_json::json!(sources);
    serde_json::to_writer_pretty(
        File::create(std::env::var("PG_PROBE_OUTPUT").unwrap()).unwrap(),
        &report,
    )
    .unwrap();
}
