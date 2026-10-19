//! Deterministic input discovery, run grouping, and exact pooled analysis.
use crate::{
    input::SummaryReader,
    integrity::{Membership, Source},
    layout::FlowcellLayout,
    metrics::Aggregator,
    report::{self, Collection, RunComparison, SourceMetadata},
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub struct AnalysisOptions {
    pub processors: usize,
    pub verify_input_content: bool,
    pub minionqc_compat: bool,
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
    first_seen: (usize, u64),
    latest_start: Option<f64>,
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
        "no sequencing_summary*.txt/tsv (or .txt.gz/.tsv.gz) files found in supplied directories"
    );
    let sources = Source::capture_all(&files, options.verify_input_content)?;
    let collection =
        inputs.len() != 1 || inputs[0].is_dir() || options.combined_only || options.split_runs;
    let indices: Vec<_> = (0..files.len()).collect();
    let mut groups = if options.processors > 1 && files.len() > 1 && !options.group_inputs {
        let components = independent_components(&files, &sources, options)?;
        eprintln!(
            "Independent input components: {}; at most {} aggregation workers",
            components.len(),
            options.processors.min(components.len())
        );
        scan_components(&components, options.processors, |indices| {
            scan_inputs(&files, &sources, indices, collection, options)
        })?
    } else {
        scan_inputs(&files, &sources, &indices, collection, options)?
    };
    groups.sort_by_key(|g| g.first_seen);
    let mut known = BTreeMap::<String, usize>::new();
    for (i, group) in groups.iter().enumerate() {
        if let Some(id) = &group.run_id {
            ensure!(
                known.insert(id.clone(), i).is_none(),
                "input run identity changed during analysis; retry with unchanged input files"
            );
        }
    }
    ensure!(
        collection || groups.len() == 1,
        "ambiguous run identity in input; use --split-runs or explicit --group-inputs"
    );
    check_sources(&sources, "after aggregation")?;
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
            .collect::<Result<Vec<_>>>()?;
        // Fit sufficient statistics on every raw observation, not bin means.
        for (file_index, path) in files.iter().enumerate() {
            let mut pass = sources[file_index].open_summary("fitted-trend pass")?;
            let mut membership = Membership::default();
            let mut reader = SummaryReader::with_options(
                &mut pass,
                options.input_schema,
                options.read_representation,
            )?;
            reader.allow_multiple_runs();
            while let Some(record) = reader.next_record()? {
                membership.observe(record.run_id);
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
            drop(reader);
            pass.finish()?;
            sources[file_index].validate_membership(membership, "fitted-trend pass")?;
        }
    }
    check_sources(&sources, "after fitted-trend passes")?;
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
    let latest_start = groups
        .iter()
        .filter_map(|g| g.latest_start)
        .reduce(f64::max);
    for (index, mut group) in groups.into_iter().enumerate() {
        let mut visualization = group.aggregator.take_minion_visualization();
        if let Some(renderer) = visualization.as_mut().filter(|v| v.needs_pass()) {
            // Only dense explicit MinION reports need this bounds-dependent pass.
            // One run's pixel grids are alive at a time, even in collections.
            for &file_index in &group.files {
                let path = &files[file_index];
                let mut pass = sources[file_index].open_summary("MinION visualization pass")?;
                let mut membership = Membership::default();
                let mut reader = SummaryReader::with_options(
                    &mut pass,
                    options.input_schema,
                    options.read_representation,
                )?;
                reader.allow_multiple_runs();
                while let Some(record) = reader
                    .next_record()
                    .with_context(|| format!("MinION visualization input {}", path.display()))?
                {
                    membership.observe(record.run_id);
                    let belongs = if options.group_inputs {
                        true
                    } else if let Some(id) = record.run_id {
                        known.get(id).copied() == Some(index)
                    } else {
                        group.run_id.is_none()
                    };
                    if belongs {
                        renderer.add(&record).with_context(|| {
                            format!("input {} line {}", path.display(), record.line)
                        })?;
                    }
                }
                drop(reader);
                pass.finish()?;
                sources[file_index].validate_membership(membership, "MinION visualization pass")?;
            }
        }
        let visualization = visualization
            .map(crate::minion::Renderer::finish)
            .transpose()?;
        let (mut report, observations) = group.aggregator.finish_retaining(group.run_id)?;
        report.diagnostics.minion_read_visualization = visualization;
        report.fitted_trends = fits.next().map(crate::trends::Plan::finish);
        if !collection {
            drop(observations);
            report.presentation = options.presentation.clone();
            configure_presentation(&mut report, options, group.latest_start)?;
            report::write(&report, output, |destinations| {
                verify_publication(&sources, &group.files, destinations)
            })?;
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
        configure_presentation(&mut report, options, group.latest_start)?;
        if !options.combined_only {
            report::write(
                &report,
                &output.join("runs").join(&source_id),
                |destinations| verify_publication(&sources, &group.files, destinations),
            )?;
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
    configure_presentation(&mut report, options, latest_start)?;
    report::write(&report, &output.join("combined"), |destinations| {
        verify_publication(&sources, &indices.iter().copied().collect(), destinations)
    })?;
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
    let name = name.strip_suffix(".gz").unwrap_or(name);
    name.contains("sequencing_summary") && (name.ends_with(".txt") || name.ends_with(".tsv"))
}

/// Preserve row/file order within each logical group, regardless of the worker.
fn scan_inputs(
    files: &[PathBuf],
    sources: &[Source],
    indices: &[usize],
    collection: bool,
    options: &AnalysisOptions,
) -> Result<Vec<Group>> {
    let mut groups: Vec<Group> = Vec::new();
    let mut known = BTreeMap::<String, usize>::new();
    for &file_index in indices {
        let path = &files[file_index];
        let mut pass = sources[file_index].open_summary("aggregation pass")?;
        let mut membership = Membership::default();
        let mut reader = SummaryReader::with_options(
            &mut pass,
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
            membership.observe(record.run_id);
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
                aggregator.enable_minion_visualization(!options.combined_only);
                groups.push(Group {
                    first_seen: (file_index, record.line),
                    latest_start: None,
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
            if !record.is_control {
                group.latest_start = Some(
                    group
                        .latest_start
                        .map_or(record.start_time, |v| v.max(record.start_time)),
                );
            }
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
                tracker.add(&record, options.cutoff)?;
            }
        }
        drop(reader);
        pass.finish()?;
        sources[file_index].validate_membership(membership, "aggregation pass")?;
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
            aggregator.enable_minion_visualization(!options.combined_only);
            aggregator.include_input_format(schema, representation)?;
            groups.push(Group {
                first_seen: (file_index, 0),
                latest_start: None,
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
    Ok(groups)
}

/// Files sharing any run ID form an indivisible component. One mixed-run file
/// stays intact; identity-free files remain explicitly labeled source units.
fn independent_components(
    files: &[PathBuf],
    sources: &[Source],
    options: &AnalysisOptions,
) -> Result<Vec<Vec<usize>>> {
    let mut parent: Vec<usize> = (0..files.len()).collect();
    fn root(parent: &[usize], mut i: usize) -> usize {
        while parent[i] != i {
            i = parent[i];
        }
        i
    }
    let mut known = BTreeMap::<String, usize>::new();
    for (i, path) in files.iter().enumerate() {
        let mut pass = sources[i].open_summary("independent-component discovery")?;
        let mut membership = Membership::default();
        let mut reader = SummaryReader::with_options(
            &mut pass,
            options.input_schema,
            options.read_representation,
        )
        .with_context(|| format!("invalid input {} during independence scan", path.display()))?;
        reader.allow_multiple_runs();
        while let Some(record) = reader
            .next_record()
            .with_context(|| format!("input {} during independence scan", path.display()))?
        {
            membership.observe(record.run_id);
            if let Some(id) = record.run_id {
                if let Some(&previous) = known.get(id) {
                    let (a, b) = (root(&parent, i), root(&parent, previous));
                    parent[a.max(b)] = a.min(b);
                } else {
                    known.insert(id.to_owned(), i);
                }
            }
        }
        drop(reader);
        pass.finish()?;
        sources[i].validate_membership(membership, "independent-component discovery")?;
    }
    let mut components = BTreeMap::<usize, Vec<usize>>::new();
    for i in 0..files.len() {
        components.entry(root(&parent, i)).or_default().push(i);
    }
    Ok(components.into_values().collect())
}

/// A bounded wave of scoped workers: no queue, copied observations or detached
/// tasks. Join every worker (also on failure) before returning any aggregators.
fn scan_components(
    components: &[Vec<usize>],
    workers: usize,
    scan: impl Fn(&[usize]) -> Result<Vec<Group>> + Sync,
) -> Result<Vec<Group>> {
    ensure!(workers > 0, "processors must be at least 1");
    let mut groups = Vec::new();
    for wave in components.chunks(workers) {
        let results = std::thread::scope(|scope| -> Result<Vec<Vec<Group>>> {
            let mut handles = Vec::new();
            for indices in wave {
                let scan = &scan;
                handles.push(
                    std::thread::Builder::new()
                        .spawn_scoped(scope, move || scan(indices))
                        .context("cannot start analysis worker; reduce --processors")?,
                );
            }
            // Join all handles before propagating the first ordered error.
            let joined: Vec<_> = handles.into_iter().map(|h| h.join()).collect();
            joined
                .into_iter()
                .map(|r| r.map_err(|_| anyhow::anyhow!("analysis worker panicked"))?)
                .collect()
        })?;
        groups.extend(results.into_iter().flatten());
    }
    Ok(groups)
}

fn configure_presentation(
    report: &mut report::Report,
    options: &AnalysisOptions,
    latest_start: Option<f64>,
) -> Result<()> {
    if !options.minionqc_compat {
        return crate::metrics::configure_mux_markers(report);
    }
    report.presentation.mux_markers_seconds.clear();
    let Some(latest) = latest_start else {
        return Ok(());
    };
    // Upstream add_cols uses integer hours, not the 15-minute bin end.
    let end_hours = (latest / 3600.0).floor();
    let interval = if report.collection.is_some() {
        8.0
    } else {
        report.presentation.mux_interval_hours.unwrap_or(end_hours)
    };
    if interval == 0.0 {
        // Upstream seq(by=0) errors on sub-hour runs. Render only the origin.
        report.presentation.mux_markers_seconds.push(0.0);
        return Ok(());
    }
    let count = (end_hours / interval).floor() + 1.0;
    ensure!(
        count <= 512.0,
        "compatibility mux interval generates more than 512 markers; increase --muxes or omit --minionqc-compat to disable references"
    );
    report.presentation.mux_interval_hours = Some(interval);
    report.presentation.mux_markers_seconds = (0..count as usize)
        .map(|i| i as f64 * interval * 3600.0)
        .collect();
    Ok(())
}

fn check_sources(sources: &[Source], stage: &str) -> Result<()> {
    sources.iter().try_for_each(|s| s.check(stage))
}
fn verify_publication(
    sources: &[Source],
    contributing: &BTreeSet<usize>,
    destinations: &[&Path],
) -> Result<()> {
    Source::reject_outputs(sources, destinations)?;
    sources.iter().enumerate().try_for_each(|(i, source)| {
        if contributing.contains(&i) {
            source.verify_for_publication()
        } else {
            source.check("publication")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn workers_overlap_are_bounded_and_join_before_error() {
        let barrier = Barrier::new(2);
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);
        let components = vec![vec![0], vec![1], vec![2], vec![3]];
        let scan = |_: &[usize]| {
            maximum.fetch_max(active.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
            barrier.wait();
            active.fetch_sub(1, Ordering::SeqCst);
            completed.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        };
        assert!(scan_components(&components, 2, scan).unwrap().is_empty());
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
        assert_eq!(completed.load(Ordering::SeqCst), 4);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        completed.store(0, Ordering::SeqCst);
        let result = scan_components(&components, 2, |indices| {
            barrier.wait();
            completed.fetch_add(1, Ordering::SeqCst);
            ensure!(indices[0] != 0, "synthetic worker failure");
            Ok(Vec::new())
        });
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("synthetic worker failure")
        );
        assert_eq!(completed.load(Ordering::SeqCst), 2);
    }
}
