# Input and run processing

Reference for supported sequencing-summary input, schema selection, run grouping, workers and source-integrity guarantees.
Start with the [quick start](../README.md#quick-start); use the [documentation index](README.md) to find metric and output definitions.

## Contents

- [Files and recursive discovery](#files-and-recursive-discovery)
- [TSV format and required columns](#tsv-format-and-required-columns)
- [Legacy representation and RNA controls](#legacy-representation-and-rna-controls)
- [Plain and gzip input](#plain-and-gzip-input)
- [Run identity and chunks](#run-identity-and-chunks)
- [Bounded parallel processing](#bounded-parallel-processing)
- [CLI compatibility settings](#cli-compatibility-settings)
- [Multi-pass input integrity](#multi-pass-input-integrity)

## Files and recursive discovery

```sh
ontrunqc analyze sequencing_summary.txt.gz --output qc_results/
ontrunqc analyze /data/flowcells/ --processors 2 --output collection_qc/
ontrunqc analyze chunk_a.tsv chunk_b.tsv.gz --output collection_qc/
```

Explicit files may have any name. Recursive discovery selects names containing
`sequencing_summary` with `.txt`, `.tsv`, `.txt.gz` or `.tsv.gz` suffixes.
Canonical paths and filesystem identities deduplicate aliases; directory symlinks
are not followed. Separately copied files or duplicate read IDs are not deduplicated.
Keep source files stable until all reports have been published.

## TSV format and required columns

Input is unquoted, tab-separated text with a header. Columns may appear in any
order; additional columns and LF/CRLF endings are accepted. Quoted/multiline TSV
fields are unsupported. Empty files fail; a valid header with no data produces a
report with zero counts and unavailable statistics. Required-field errors include
source and decompressed line context; malformed records are never silently skipped.

| Mode | Required fields | Validation |
| --- | --- | --- |
| Modern | `run_id`, `channel`, `mux`, `start_time`, `duration`, `sequence_length_template`, `mean_qscore_template`, `end_reason` | Finite nonnegative time/duration/Q; valid channel/mux and numeric length. |
| Legacy | `channel`, `start_time`, a complete template or 2D length/Q pair | Finite signed Q accepted; absent modern metadata stays unavailable. |
| Auto (default) | Selects from the header and read representation | Modern rules apply when the modern template schema is present; otherwise legacy rules apply. |

`passes_filtering` is optional; case-insensitive TRUE/FALSE is accepted. Missing
or incomplete flags make native filtering unavailable, never inferred from Q.
Optional `template_duration` affects only the speed proxy; invalid/nonpositive
values make that observation ineligible for speed, without falling back to `duration`.

Use `--input-schema modern|legacy|auto` and
`--read-representation template|2d|auto` to select behavior explicitly.

## Legacy representation and RNA controls

`--input-schema auto` uses the modern schema when all eight original modern
columns and template representation are present; otherwise it selects legacy.
`--input-schema modern` enforces the original required columns and nonnegative Q;
`legacy` permits missing run/mux/duration/native/reason metadata. Any provided
numeric metadata is still validated. Legacy blanks/NA in optional metadata are
unavailable; selected length/Q and channel/start remain required. Missing muxes
and reasons create no invented groups. A partially missing native flag makes
native completeness false while known TRUE/FALSE counts remain observed counts.

Auto representation chooses the full `sequence_length_2d`/`mean_qscore_2d` pair
when either is present (both are required). `--read-representation template|2d`
makes the choice explicit. There is no row-level fallback; incompatible
representations within a run are rejected. Each collection member records its
representation; pooled comparisons may explicitly contain different run types.
Template duration, template native flags and template event counts are unavailable
for 2D reads. Finite signed legacy Q values are retained without clipping; when
needed, an explicit [-f64::MAX,0) quality underflow interval and joint-histogram
row are prepended. Nonnegative modern histogram boundaries are unchanged. Signed
means/medians use overflow-safe arithmetic; time plots support signed axes.

`input` records raw validated row counts, exact `YHR174W` control exclusions,
negative-Q record counts, schemas and chosen representation. Control rows are
validated and excluded before QC aggregation; excluded rows do not contribute
zero-length/basecalled/filtering metrics. This reproduces MinIONQC's named-control
policy, not a universal RNA filtering recommendation. Source rows with malformed
required values still fail, unlike upstream's silent malformed-row removal.
Legacy blank/`NA` calibration labels are unknown, so they are retained. Pinned
MinIONQC's `subset(label != "YHR174W")` also drops missing labels: a synthetic
10/20/30/40-bp missing/NA/ordinary/control fixture retains 3 reads/60 bp here and
1 read/30 bp in actual R. This intentional population difference is tested; no
unknown label is inferred to be a control.

## Plain and gzip input

Compression changes transport only, not the TSV parser, populations, statistics
or JSON schema. Recursive discovery accepts names containing `sequencing_summary`
ending in `.txt`, `.tsv`, `.txt.gz` or `.tsv.gz`. Existing canonical/inode alias
removal and directory-symlink protections are unchanged. Explicit paths may have
any name: gzip magic bytes select gzip decoding; a canonical filename ending in
`.gz` always requires gzip and a mislabeled plain file is rejected. Suffixes are
case-sensitive for discovery. Source ordering ignores the final `.gz` suffix,
with full canonical paths breaking ties; uncompressed ordering is unchanged.
Compressed and plain chunks can share a run ID.

`Source::open_summary` supplies one reader for discovery, aggregation, raw-fit,
reference histogram/temporal and dense MinION passes, and strong publication verification. It uses a 64-KiB
source buffer and `flate2`'s multi-member gzip decoder, with the existing pure
Rust backend. All members (including empty ones), CRC32 and size trailers must
validate. Invalid headers, truncated streams and any invalid trailing bytes
(including zero padding) are errors with source/pass context. A concatenation is
one decompressed TSV, with one header, not separately parsed summary files.
An empty gzip payload has the same empty-input error as an empty plain file;
header-only content remains valid. Parser line numbers refer to decompressed TSV.

The raw source guard sits below buffering/decompression. File identity, size,
metadata checkpoints, consumed-byte count and optional SHA-256 all refer to the
**compressed file bytes**, including headers and trailers, not decoded text.
Finalization requires error-free decoded EOF, no unread buffered source bytes
and raw-file EOF before saving/checking a digest. Reading valid TSV rows without
consuming the final gzip trailer is insufficient. An error cannot be cleared by
reading EOF after a decoder failure. Changed gzip metadata bytes are detectable
in strong mode even when the decompressed TSV is identical.

No decoded TSV is buffered in full or extracted to disk. Every additional pass
reopens and decompresses the source, including strong publication rereads; gzip
may save storage/I/O while adding CPU. It does not remove exact-statistics memory
requirements. Metadata/NFS limitations below still apply. `flate2` is now a core
dependency, also available with `--no-default-features`; no dependency versions
or numerical reference targets changed. See [measurements](benchmark.md#streaming-gzip-input).

## Run identity and chunks

Directory/multiple-file analysis routes each record by actual `run_id` in one
streaming pass. Ordinary single-file analysis retains strict run-ID validation;
`--split-runs` explicitly enables splitting a mixed-run file. Run IDs can recur
across files (chunks); flowcell IDs must be consistent within a run. Distinct run
IDs do not necessarily identify distinct physical flowcells. Missing identity is
reported as a separate source unless explicitly grouped. `--group-inputs` cannot
merge conflicting known IDs. Output paths use deterministic ordinal identifiers,
never sample or instrument identifiers. Canonical/inode deduplication covers path
aliases/hard links, not duplicate read IDs or separately copied overlapping chunks.

```sh
ontrunqc analyze mixed_runs.tsv --split-runs --output split_qc/
ontrunqc analyze source_a.tsv source_b.tsv --group-inputs --output grouped_qc/
ontrunqc analyze /data/flowcells/ --combined-only --output pooled_qc/
```

Output structure and pooled statistics are described in [reports and exports](output.md#single-run-and-collection-output)
and [multi-run metrics](metrics.md#multi-run-statistics).

## Bounded parallel processing

`--processors 1` preserves the original parse/reduce order. For N>1 with
multiple files and no explicit grouping, one additional validating streaming
pass records run IDs and joins files with overlapping IDs into indivisible
components. Distinct components run in bounded waves of at most N scoped
threads. There is no queue or copied observation dataset. A mixed-run file
stays within one worker; chunks sharing an ID stay in file/row order. Legacy
files without IDs remain independent **source labels**, not invented runs.
Single inputs and `--group-inputs` bypass the independence scan.

Groups are restored to first-seen `(file index, record line)` order. Floating
updates within each run, global fit-method choice, shared KDE domain, exact
statistics and pooled reductions retain their original deterministic order.
Fitted-trend rescans, dense MinION rescans/finalization and all report rendering
remain sequential. Bounded workers are not a whole-process memory bound:
all exact numeric observations remain owned once by their run aggregators,
plus active parser buffers/stacks and existing sorting/pooling allocations.
No numerical fitting/KDE or MinION raster implementation is changed.

All workers finish/join before any report publication. Worker failure produces
an error and no new report outputs; previous reports remain valid. Later render
or rename failures retain existing per-file staging guarantees, **not** a
collection-wide transaction. Inputs must remain unchanged across passes;
new conflicting identities detected after discovery fail before publication.
Directory/path/inode deduplication is unchanged; separately copied duplicate
reads are not deduplicated.

`tests/parallel_runs.py` optionally compares the prior binary, one worker and
two workers on synthetic/public inputs, dense MinION fits/KDE and all six static
formats. JSON/HTML, all non-PDF figures, source paths and manifests must match
byte-for-byte; PDF resource ordering remains the documented #15 exception.
See [executed measurements](benchmark.md#bounded-independent-run-processing-16).

## CLI compatibility settings

Positional input remains the normal syntax; `-i/--input` is an alias. Output uses
`-o/--output`, with `--outputdirectory` accepted for historical commands.
The [option-by-option audit](minionqc-comparison.md#cli-defaults-and-output-behavior)
distinguishes compatible aliases, safe defaults and remaining differences.

Logical `--combined-only`, `--smallfigures`/`--small-figures`, `--add-stat` and
short aliases accept bare flags, `=TRUE/FALSE`, or spaced `TRUE/FALSE` (case
insensitive). Only those exact logical tokens are normalized; other paths are
not consumed as option values. `--` terminates option processing. Use equals
syntax or `--` to disambiguate a literal input filename named TRUE/FALSE.

Q cutoffs accept finite signed numbers, inclusively. Negative cutoffs can select
legacy negative-Q observations; they never bypass modern nonnegative input-Q
validation. Selected quality histograms then prepend the same explicit
`[-f64::MAX,0)` underflow interval used by the existing all-read histogram.
Pooling adds counts by bin boundary, conserving selected counts; existing
nonnegative bin boundaries and all previously supported-cutoff results remain
unchanged. No cutoff is substituted for native filtering.

`--minionqc-compat` is opt-in. `presentation.minionqc_compatibility=true` appears
only for that mode (schema 1 additive presentation metadata). It uses upstream's
`hour=floor(start_time/3600)` convention for mux reference extents. Single-run
`--muxes 0` uses the maximum observed integer hour, positive values define
spacing, and combined comparisons use 8 hours regardless of the individual
setting. Controls do not determine the extent. Empty inputs have no references;
sub-hour runs get only the origin instead of reproducing R's zero-step `seq`
error. The 512-marker guard remains. These are never instrument telemetry.
Normal defaults and bin-end-based explicit references remain unchanged.

Omitted `--output` is accepted only in compatibility mode with one input path.
A fresh `ontrunqc_qc/` is atomically reserved beside that file or within that
directory. Existing destinations, including symlinks, are refused; an empty
reservation is removed on failure. Completed per-run outputs are never deleted
on later failure. Multiple paths need explicit output. This deliberately differs
from upstream's direct beside-input writes. Default output remains HTML/JSON;
PNG requires `--format png`, including in compatibility mode/minimal builds.

## Multi-pass input integrity

Before processing, canonical regular-file sources are frozen with byte length,
mtime and (Unix) device/inode plus nanosecond ctime. Identity collisions detected
while freezing fail rather than double-counting sources. Original symlink aliases
are resolved once; the canonical target is the source being analyzed. Directory
contents added after discovery are not automatically included.

Every source open compares path/canonical identity and the open descriptor.
Streaming reads recheck metadata each 8 MiB of source bytes; EOF/finalization checks metadata,
complete EOF and consumed byte count. Discovery, aggregation, GAM input,
reference histogram/temporal and dense MinION passes all use this guard. Each completed parsed pass also compares its
run-ID/count map, including missing-ID and control rows; no per-record strings
are retained. Checks between phases and after rendering/before the first staged
rename reject detected drift with source/stage and stable-copy guidance. Actual
output destinations are checked against every canonical source path and Unix
file identity: reports cannot overwrite their own inputs (including inputs named
`report.json` or a generated figure name). Failure
cleans current staged outputs; prior per-run outputs are not rolled back.
Publication is per file, not atomic across a run's files or a collection.

`--verify-input-content` adds incremental SHA-256 over the exact bytes consumed
by every processing pass, using the first completed pass as baseline. After each
report is rendered it rereads that report's contributing files and compares the
same digest before publication; gzip is decoded/validated again while hashing its
compressed bytes. The combined report verifies all sources.
Mixed-run sources/chunks can consequently be reread for several reports. This
is explicit extra I/O, without full-file buffering; ordinary metadata mode adds
no data scan. All numerical report fields/JSON are unchanged, and no digest,
source path or new research metadata is embedded. The small MIT/Apache-2.0
`sha2` dependency avoids handwritten cryptography/external checksum commands;
existing dependency versions remain locked. Minimal builds support the guard.

Metadata is not a filesystem snapshot. Coarse/changed-back timestamps, delayed
NFS attribute caches, inode reuse and in-place mutations that escape checkpoints
can defeat metadata-only checks; ctime may also conservatively flag chmod/link
operations without changed bytes. SHA-256 checks observed byte streams, not an
atomic server-side file version: data caches can hide remote changes, and writes
restored between checks or after final verification remain possible. Do not use
advisory locking to imply remote-writer cooperation. Use completed files, a
stable local copy or storage snapshots for stronger guarantees. Actual NFS
behavior was not exercised locally. No automatic retry mixes file versions.

Filesystem background: [Rust metadata](https://doc.rust-lang.org/std/fs/struct.Metadata.html)
and [Linux NFS cache/coherence semantics](https://man7.org/linux/man-pages/man5/nfs.5.html).
The checks above report observed consistency; they do not provide cache bypass,
remote-writer coordination or atomic source snapshots.


### Optional reference temporal pass

`--minionqc-time-series` performs one additional guarded read per run/file
membership. Mixed-run files may therefore be decoded once per affected run.
Only bounded numeric sort records are written to private `TMPDIR` scratch; no
TSV extraction occurs. Sorting and rendering remain sequential after bounded
aggregation workers. [Temporal definitions and scratch/error limits](metrics.md#optional-minionqc-temporal-plots-20)
apply independently of native diagnostics and the other compatibility flags.
