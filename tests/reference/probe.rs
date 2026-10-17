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
    let mut aggregator = Aggregator::new(None, false);
    aggregator.set_qscore_cutoff(cutoff, true);
    for path in paths {
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
        }
    }
    let report = aggregator.finish(None).unwrap();
    serde_json::to_writer_pretty(
        File::create(std::env::var("PG_PROBE_OUTPUT").unwrap()).unwrap(),
        &report,
    )
    .unwrap();
}
