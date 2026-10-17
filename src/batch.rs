//! Deterministic input discovery, run grouping, and exact pooled analysis.
use crate::{
    input::SummaryReader,
    layout::FlowcellLayout,
    metrics::Aggregator,
    report::{self, Collection, RunComparison, SourceMetadata},
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    path::{Path, PathBuf},
};

pub struct AnalysisOptions {
    pub layout: Option<FlowcellLayout>,
    pub cutoff: f64,
    pub minionqc_trends: bool,
    pub minionqc_density: bool,
    pub combined_only: bool,
    pub group_inputs: bool,
    pub split_runs: bool,
    pub input_schema: crate::input::InputSchema,
    pub read_representation: crate::input::ReadRepresentation,
    pub presentation: crate::report::PresentationOptions,
}

struct Group {
    files: BTreeSet<usize>,
    run_id: Option<String>,
    flow_cell_id: Option<String>,
    grouping: &'static str,
    aggregator: Aggregator,
    trends: Option<crate::trends::Tracker>,
}

pub fn analyze(inputs: &[PathBuf], output: &Path, options: &AnalysisOptions) -> Result<()> {
    ensure!(
        !inputs.is_empty(),
        "provide an input file/directory or --input PATH"
    );
    let files = discover(inputs)?;
    ensure!(
        !files.is_empty(),
        "no sequencing_summary*.txt/tsv files found in supplied directories"
    );
    let collection =
        inputs.len() != 1 || inputs[0].is_dir() || options.combined_only || options.split_runs;
    let mut groups: Vec<Group> = Vec::new();
    let mut known = BTreeMap::<String, usize>::new();
    for (file_index, path) in files.iter().enumerate() {
        let mut reader = SummaryReader::with_options(
            File::open(path).with_context(|| format!("cannot open input {}", path.display()))?,
            options.input_schema,
            options.read_representation,
        )
        .with_context(|| format!("invalid input {}", path.display()))?;
        if collection {
            reader.allow_multiple_runs();
        }
        let (native, speed) = (
            reader.has_native_filtering(),
            reader.has_template_duration(),
        );
        let (schema, representation) = (reader.schema(), reader.representation());
        let mut fallback = None;
        let mut had_record = false;
        while let Some(record) = reader
            .next_record()
            .with_context(|| format!("input {}", path.display()))?
        {
            had_record = true;
            let run_id = record.run_id;
            let index = if options.group_inputs && !groups.is_empty() {
                Some(0)
            } else if let Some(id) = run_id {
                known.get(id).copied()
            } else {
                fallback
            };
            let index = index.unwrap_or_else(|| {
                let index = groups.len();
                let mut aggregator = Aggregator::new(options.layout, false);
                aggregator.set_qscore_cutoff(options.cutoff, true);
                aggregator.enable_reference_density(options.minionqc_density);
                aggregator.set_sampling_enabled(!options.combined_only);
                groups.push(Group {
                    trends: options
                        .minionqc_trends
                        .then(crate::trends::Tracker::default),
                    files: BTreeSet::new(),
                    run_id: run_id.map(str::to_owned),
                    flow_cell_id: None,
                    grouping: if options.group_inputs {
                        "explicit_group"
                    } else if run_id.is_some() {
                        "run_id"
                    } else {
                        "unidentified_source"
                    },
                    aggregator,
                });
                if let Some(id) = run_id {
                    known.insert(id.to_owned(), index);
                } else {
                    fallback = Some(index);
                }
                index
            });
            let group = &mut groups[index];
            ensure!(
                run_id.is_none() || group.run_id.is_none() || group.run_id.as_deref() == run_id,
                "--group-inputs cannot merge different known run_id values"
            );
            ensure!(
                record.flow_cell_id.is_none()
                    || group.flow_cell_id.is_none()
                    || record.flow_cell_id == group.flow_cell_id.as_deref(),
                "input {} line {}: conflicting flow_cell_id within the same run/group",
                path.display(),
                record.line
            );
            if group.run_id.is_none() {
                group.run_id = run_id.map(str::to_owned);
            }
            if group.flow_cell_id.is_none() {
                group.flow_cell_id = record.flow_cell_id.map(str::to_owned);
            }
            if group.files.insert(file_index) {
                group.aggregator.include_input_capabilities(native, speed);
                group
                    .aggregator
                    .include_input_format(schema, representation)?;
            }
            group
                .aggregator
                .add(&record)
                .with_context(|| format!("input {} line {}", path.display(), record.line))?;
            if let Some(tracker) = &mut group.trends {
                tracker.add(&record, options.cutoff);
            }
        }
        if !had_record && options.group_inputs && !groups.is_empty() {
            let group = &mut groups[0];
            group.files.insert(file_index);
            group.aggregator.include_input_capabilities(native, speed);
            group
                .aggregator
                .include_input_format(schema, representation)?;
        } else if !had_record {
            let mut aggregator = Aggregator::new(options.layout, speed);
            aggregator.set_qscore_cutoff(options.cutoff, native);
            aggregator.enable_reference_density(options.minionqc_density);
            aggregator.set_sampling_enabled(!options.combined_only);
            aggregator.include_input_format(schema, representation)?;
            groups.push(Group {
                trends: options
                    .minionqc_trends
                    .then(crate::trends::Tracker::default),
                files: BTreeSet::from([file_index]),
                run_id: None,
                flow_cell_id: None,
                grouping: "empty_source",
                aggregator,
            });
        }
    }
    ensure!(
        collection || groups.len() == 1,
        "ambiguous run identity in input; use --split-runs or explicit --group-inputs"
    );
    let mut plans = Vec::new();
    if options.minionqc_trends {
        let largest = groups
            .iter()
            .filter_map(|g| g.trends.as_ref())
            .map(crate::trends::Tracker::count)
            .max()
            .unwrap_or(0);
        plans = groups
            .iter_mut()
            .map(|g| g.trends.take().unwrap().prepare(largest >= 1000))
            .collect();
        // Fit sufficient statistics on every raw observation, not bin means.
        for (file_index, path) in files.iter().enumerate() {
            let mut reader = SummaryReader::with_options(
                File::open(path)?,
                options.input_schema,
                options.read_representation,
            )?;
            reader.allow_multiple_runs();
            while let Some(record) = reader.next_record()? {
                let index = if options.group_inputs {
                    Some(0)
                } else if let Some(id) = record.run_id {
                    known.get(id).copied()
                } else {
                    groups
                        .iter()
                        .position(|g| g.run_id.is_none() && g.files.contains(&file_index))
                }
                .with_context(|| {
                    format!(
                        "input {} changed identity during fitted-trend pass",
                        path.display()
                    )
                })?;
                plans[index].add(&record, options.cutoff);
            }
        }
    }
    let mut fits = plans.into_iter();
    let mut domain = crate::density::Bounds::default();
    if options.minionqc_density {
        for group in &groups {
            domain.merge(group.aggregator.density_bounds());
        }
        for group in &mut groups {
            group.aggregator.set_density_domain(domain);
        }
    }
    let mut pooled = Aggregator::new(None, false);
    pooled.set_qscore_cutoff(options.cutoff, true);
    pooled.enable_reference_density(options.minionqc_density);
    pooled.set_density_domain(domain);
    let mut runs = Vec::new();
    for (index, group) in groups.into_iter().enumerate() {
        let (mut report, observations) = group.aggregator.finish_retaining(group.run_id)?;
        report.fitted_trends = fits.next().map(crate::trends::Plan::finish);
        if !collection {
            drop(observations);
            report.presentation = options.presentation.clone();
            crate::metrics::configure_mux_markers(&mut report)?;
            report::write(&report, output)?;
            return Ok(());
        }
        let source_id = format!("run_{:03}", index + 1);
        report.source = Some(SourceMetadata {
            source_id: source_id.clone(),
            input_files: group.files.len(),
            grouping: group.grouping,
            flow_cell_id: group.flow_cell_id,
        });
        report.presentation = options.presentation.clone();
        crate::metrics::configure_mux_markers(&mut report)?;
        if !options.combined_only {
            report::write(&report, &output.join("runs").join(&source_id))?;
        }
        runs.push(RunComparison::from_report(&report));
        pooled.absorb(&report, observations)?;
    }
    let mut report = pooled.finish(None)?;
    report.collection = Some(Collection {
        unit: "sequencing_runs_or_explicit_sources",
        runs,
    });
    report.presentation = options.presentation.clone();
    crate::metrics::configure_mux_markers(&mut report)?;
    report::write(&report, &output.join("combined"))?;
    Ok(())
}

fn discover(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut candidates = BTreeSet::new();
    for path in inputs {
        let metadata =
            fs::metadata(path).with_context(|| format!("cannot open input {}", path.display()))?;
        if metadata.is_dir() {
            visit(path, &mut candidates)?;
        } else {
            ensure!(
                metadata.is_file(),
                "input {} is not a regular file",
                path.display()
            );
            candidates.insert(fs::canonicalize(path)?);
        }
    }
    // Canonical paths remove symlink aliases; inode identity additionally removes
    // hard links on supported Unix targets. File names never become run metadata.
    let mut identities = BTreeSet::new();
    Ok(candidates
        .into_iter()
        .filter(|path| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                fs::metadata(path)
                    .map(|m| identities.insert((m.dev(), m.ino())))
                    .unwrap_or(true)
            }
            #[cfg(not(unix))]
            {
                identities.insert(path.clone())
            }
        })
        .collect())
}

fn visit(directory: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("cannot read directory {}", directory.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("cannot list directory {}", directory.display()))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let kind = entry
            .file_type()
            .with_context(|| format!("cannot inspect {}", path.display()))?;
        if kind.is_dir() {
            visit(&path, files)?;
        } else if supported_name(&entry.file_name().to_string_lossy())
            && (kind.is_file() || kind.is_symlink())
        {
            let metadata = fs::metadata(&path)
                .with_context(|| format!("cannot inspect summary {}", path.display()))?;
            if metadata.is_file() {
                files.insert(fs::canonicalize(&path)?);
            }
        }
        // Never descend into directory symlinks, including cycles.
    }
    Ok(())
}

fn supported_name(name: &str) -> bool {
    name.contains("sequencing_summary") && (name.ends_with(".txt") || name.ends_with(".tsv"))
}
