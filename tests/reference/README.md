<a id="optional-executable-minionqc-reference"></a>

# MinIONQC reference validation

Optional developer validation against original MinIONQC; normal ONTRunQC analysis never requires R.
[Documentation index](../../docs/README.md) · [Scientific comparison](../../docs/minionqc-comparison.md) · [Evidence ledger](../../docs/minionqc-parity.md)

Pinned reference: MinIONQC commit `54485d586038d8ee36f168dd94f31c423f52f581`, R 4.4.1, ggplot2 3.5.1, mgcv 1.9-1. Original strict GAM failures remain failures; these instructions do not waive or replace numerical targets.

## Contents

- [Select a validation task](#select-a-validation-task)
- [Prepare the reference source](#prepare-the-reference-source)
- [Reference environment](#reference-environment)
- [Run the paired suite](#run-the-paired-suite)
- [Formula and visual layer audit (#18)](#formula-and-visual-layer-audit-18)
- [Comparison populations and grouping](#comparison-populations-and-grouping)
- [Tolerances and numerical capture](#tolerances-and-numerical-capture)
- [Golden fixtures and provenance](#golden-fixtures-and-provenance)
- [Final GAM/LOESS assessment](#final-gamloess-assessment)
- [Independent-backend stability assessment](#independent-backend-stability-assessment)
- [LOESS reference and notice](#loess-reference-and-notice)
- [Input-integrity repetition (2026-10-19)](#input-integrity-repetition-2026-10-19)

## Select a validation task

Run commands from the repository root after a locked release build. Optional
reference work never installs R automatically and does not affect the normal CLI.

| Task | Command / section | Dependencies |
| --- | --- | --- |
| Fast harness tests | `python3 -m unittest discover -s tests/reference` | Python standard library. |
| Documentation links | `python3 tests/check_docs.py` | Python standard library. |
| Public statistics / saved upstream YAML | `python3 tests/minionqc_reference.py /path/to/reference` | Clean pinned source/fixtures; release binary. No R execution. |
| Actual paired reference | [Run the paired suite](#run-the-paired-suite) | Pinned R/packages or explicitly chosen container. |
| Actual plot-layer/gallery comparison | [Formula and visual audit](#formula-and-visual-layer-audit-18) | Pinned R, normal static-figures build, ImageMagick. |
| Strict evidence repeat | [Revalidation](#input-integrity-repetition-2026-10-19) | Completed paired `comparison.json`; no fresh R run by the checker. |
| Diagnose GAM/LOESS | [Controlled GAM](#final-gamloess-assessment), [LOESS goldens](#loess-reference-and-notice) | Pinned R and the named retained artifacts. |
| Dense MinION / export / worker checks | `tests/minion_visualization.py`, `static_exports.py`, `parallel_runs.py` | See each script's `--help` and [measurements](../../docs/benchmark.md). |

## Prepare the reference source

Use the clean MinIONQC v1.4.1 checkout, including public fixtures:

```sh
git clone https://github.com/roblanf/minion_qc /tmp/minionqc-reference
git -C /tmp/minionqc-reference checkout 54485d586038d8ee36f168dd94f31c423f52f581
cargo build --release --locked
```

The harness verifies the source commit and clean working tree. Keep downloaded
examples and generated reports outside Git. No private input is required.

## Reference environment

Use an existing R installation; no environment is created by this command. There
is no automatic R CI build. The optional Dockerfile remains an alternative for
users who explicitly want a container: build it with `docker build -t
ontrunqc-minionqc-r -f tests/reference/Dockerfile tests/reference`, then replace
`--rscript /path/to/Rscript` with `--docker ontrunqc-minionqc-r`.

The reference environment is R **4.4.1**, ggplot2 **3.5.1**, mgcv **1.9-1**, with
transitive R dependencies from the **2024-07-01** CRAN snapshot. These versions
are an explicitly selected reproducible environment, not a claim about the
unrecorded package versions originally used by MinIONQC's author. The image tag
and OS package repositories are not bit-for-bit immutable: record the resolved
Docker image ID/digest alongside the capture's full package versions/sessionInfo
when archiving results. Native execution supports `--rscript /path/to/Rscript`
with the same checked versions. Locale/timezone are fixed in the image; seed is 1.
Before any case, a native/container preflight checks the **installed** R,
ggplot2 and mgcv versions and saves `reference-environment.json` plus session and
package versions. A version failure returns 2 with `preflight.log`; an environment
name never substitutes for these checks. Each completed case checkpoints the
comparison report, records R's exit status and capture SHA-256, and distinguishes
execution from successful completion. An interrupted suite retains prior cases.

## Run the paired suite

```sh
python3 tests/minionqc_reference.py /tmp/minionqc-reference --execute-r \
  --rscript /path/to/Rscript --capture-only \
  --output benchmark-results/minionqc-r
```

Omit `--capture-only` to render original PDF reports as well. Use a fresh output
folder for a new execution. Choose `--case short` to inspect a single case.

This runs the unmodified script on unchanged source bytes, with matching Q
cutoffs, one processor, explicit 8-hour mux references and PDF figures. The `ggsave` wrapper also calls
`ggplot_build()` and records numerical layers, method/formula, panels, legends
and axis labels. `--capture-only` skips figure rendering but still runs upstream
summary/plot calculations. Full runs include
MinION, PromethION, two MinION inputs/combined QC, short, 1D2, RNA and synthetic
zero/cutoff/N50/ultralong edge cases, small/large/irregular/sparse fits and
multimodal/skewed/constant/near-constant/outlier densities and singleton groups
with unequal source sizes (18 cases). Nothing uses private sequencing data.

Each case saves original R YAML/PDF, capture JSON, ONTRunQC JSON/YAML, plot
inventory and logs. `comparison.json` includes source/input hashes, Q cutoffs,
reference values, differences, tolerances and six explicit status categories.
Unknown mismatches are defects requiring investigation, never accepted silently.
Counts/bases use exact comparisons; one-decimal summaries allow 0.050000001;
serialized Gb yields allow 50.1 bp. Zero-denominator/N50 tie differences are named;
the upstream ultralong endpoint bug requires a numerical proof before classification.
Runs return 1 for suspected ONTRunQC defects, 2 for unverified execution, 0 only
when every requested comparison is accounted for. Missing R never generates
invented reference values. R's `quit()` or partial output may prevent comparisons;
inspect logs and retain Not yet verified until the cause is established.

### Exit status and artifacts

| Status | Meaning |
| --- | --- |
| 0 | Requested comparisons were accounted for under their stated classification; inspect intentional/upstream differences. |
| 1 | Suspected native discrepancies remain. The retained 18-case baseline exits 1 for 1,542 GAM checks. |
| 2 | Execution or reference evidence remains unverified, including missing/wrong reference dependencies. |

Each case directory contains input copies, R/native logs, capture JSON, summaries
and plot inventory. `comparison.json` is checkpointed across cases and records
values, tolerances, status, explanations and source/input hashes. Classification
is per check, not a count of independent bugs.

## Formula and visual layer audit (#18)

The separate optional tool observes existing built layers, with no changes to
strict smoothing/density targets or normal R captures:

```sh
python3 tests/reference/plot_audit.py /path/to/pinned/minion_qc \
  --rscript /path/to/Rscript
Rscript tests/reference/plot_layers_test.R
```

Requires the existing pinned R environment, normal Cargo static-figures build
and ImageMagick (`magick`); Python uses only its standard library. No environment
is installed. Six public cases generate 85 actual R plots, native canonical PNG
exports, bounded layer captures, numeric diagnostics and representative A/B/C
cards. Use `--case promethion`, or `--reuse` for matching input/source hashes.
Full reports live in ignored `benchmark-results/plot-audit/`. The small gallery,
aggregate checks, input/source hashes and R sessions are in
`docs/figures/minionqc-comparison/`. Original PNG is rendered at 60 DPI for this
visual audit; native exports are 300 DPI, with gallery-only resize/palette
reduction. These device changes do not change plot calculations.

`PG_PLOT_AUDIT=true` plus `PG_PLOT_AUDIT_HELPER=.../plot_layers.R` enables bounded
capture of StatBin/StatBin2d/StatCount, spatial tiles and full finite-row grids.
Exact histogram boundaries are already transformed (log10 length where used).
Large row samples are labelled samples, never complete plotted populations.
Cumulative queries count all reads tied at a queried start time. Spatial yield is
captured before mapping fill to colors. Missing/nonfinite/clipped rows remain
explicit. `plot_checks.py` independently checks counts against original TSV
populations and quantifies display interpolation separately from exact queries.

Different bins are not interpolated/subtracted as a false residual. Diagnostic
residuals use only matching X coordinates; the original strict suite still
checks every fit/uncertainty group. A gallery first-group comparison cannot waive
an unresolved Q-selected GAM discrepancy. Gallery execution returns nonzero for
unexplained numeric differences; different aggregation/population is named.
The production CLI is exercised separately by the paired differential suite.

The existing ignored probe optionally writes actual canonical reports/figures
with `PG_PROBE_FIGURES`; these explicitly use upstream file scope, not invented
instrument identities. Minimal builds are not used for native PNG validation.

## Comparison populations and grouping

Public files can contain several run IDs. MinIONQC groups by parent directory;
ONTRunQC correctly groups by run ID. Pooled statistics are compared on identical
file populations; source/file versus instrument identity is not silently equated.
Saved upstream YAML remains a separate fast independent check (omit `--execute-r`).
Harness logic tests: `python3 -m unittest discover -s tests/reference`.

The ignored Rust `reference_probe::export_report` test reuses the actual parser and
aggregator on the upstream file-scoped population for channel/curve comparisons.
This is necessary because upstream drops run IDs before analysis. Normal CLI
reports are also generated and checked; production run grouping is never changed.
Probe artifacts are validation-only and cannot be mistaken for instrument metadata.

Historical `poregauge` comparison keys, status names,
output subdirectories and explanation strings are retained by the differential
harness so original numerical evidence and check signatures remain verifiable.
Run it with `target/release/ontrunqc`; no reference targets or tolerances changed.

## Tolerances and numerical capture

For numerical layers, the validation-only probe fits the exact upstream file
populations and computes reference densities on shared domains. `capture.json`
retains all `StatSmooth`/`StatDensity` points; other layers keep metadata/row counts,
not complete read point arrays. Comparison targets: hours absolute 1e-10, fitted
values absolute 1e-6 + relative 1e-6; densities absolute max(peak×1e-7,1e-12) +
relative 1e-6, plotted-domain integrals absolute 1e-7 + relative 1e-6, peaks within
one grid interval. Actual bandwidths/domains/kernel/grid size are captured inside
ggplot2's `compute_density()` without changing its returned values. Bandwidth
tolerance is absolute 1e-12 + relative 1e-10. The density targets have now been
met on all executed public/synthetic density groups; fitting remains Partial.
GAM pointwise standard errors and 95% bounds are compared at the fitted-value
tolerance; LOESS intervals are also compared, with nonfinite reference bands
checked as unavailable rather than fabricated numerical matches. Density
integrals are over the plotted domain; unplotted Gaussian tails are not silently
renormalized to force area one. Constant-domain evaluation is at the unchanged
single coordinate. A `predictdf()` exit trace also captures uncensored predictions,
observation counts, GAM knots, smoothing parameters and residual variance before
plot-axis clipping. Comparisons use these predictions when available, including
negative fitted values hidden by an axis limit. Nonfinite reference predictions
remain Not yet verified; null is not numerical evidence.
Failed point/method comparisons are never automatically accepted.

## Golden fixtures and provenance

`trends-golden.json` is a small, synthetic-only numerical snapshot of the actual
`predictdf`/`StatSmooth` values from `fit-small`, `fit-large`, `zero-cutoff`,
`n50-tie` and `ultralong-endpoint`, including
their exact TSV bytes, reference versions and capture SHA-256. The Rust unit test
re-runs parsing/fitting and compares all 1,598 finite fitted values and 960
large-case GAM uncertainty values at the same tolerance. It is not evidence
that the other paired cases pass. The latest paired run is recorded in
`validation-summary.json`; the full ignored report is
`benchmark-results/parity-final/comparison.json`. Its exit 1 reflects remaining
MinION selected/combined fits. The PromethION endpoint regression is corrected.

`density-golden.json` contains 24 synthetic series (22 finite curves and two
singleton unavailable states), exact observations, actual layer coordinates and
traced bandwidths, reference versions and capture hashes. Identical All/Q curves
are stored once. Rust tests check all 11,264 density points, domains, bandwidths,
integrals and peaks. Full paired results still include the distinct All/Q series.

GAM intermediate capture includes natural-cubic F/S, raw eigenvectors/values,
shrunk penalties, prediction design values, model-to-nodal transforms, model
crossproducts, REML/optimizer diagnostics, edf, residual variance, coefficients
and posterior covariance. The harness enables the validation-only Rust audit;
`gam_diagnostics` compares equivalent nodal quantities separately from primary
plot checks. Null eigenspaces are compared by projectors, not arbitrary signed
vectors. Diagnostic matrix tolerances are stated per scalar in the report;
existing curve/SE/bound tolerances remain absolute 1e-6 + relative 1e-6.

`gam-golden.json` stores only aggregate sufficient statistics from the public
upstream PromethION example (input/capture hashes included), plus actual R
basis/penalty/prediction values for four models. It contains no individual reads,
research/sample identifiers or private inputs. Attribution: public examples from
Rob Lanfear's MinIONQC repository at the pinned commit (MIT licensed). A native
Rust test reconstructs the model and checks the formerly 211 bp endpoint,
all fitted values and uncertainties. Analytical REML derivatives also have an
independent finite-difference test.

## Final GAM/LOESS assessment

Run the controlled experiment **separately** from the unchanged reference suite:

```sh
/path/to/Rscript tests/reference/gam_assessment.R \
  benchmark-results/parity-final benchmark-results/gam-controlled-final.json
/path/to/Rscript tests/reference/cs_sensitivity.R \
  benchmark-results/parity-final/minion/inputs/source_001/sequencing_summary.txt \
  7 benchmark-results/cs-final-sensitivity.json
python3 tests/reference/assess_fits.py \
  benchmark-results/gam-paired/comparison.json \
  benchmark-results/parity-final/comparison.json \
  benchmark-results/gam-controlled-final.json \
  benchmark-results/cs-final-sensitivity.json \
  benchmark-results/final-fit-assessment.json
```

The first comparison argument is the retained 406e354 baseline. The summarizer
asserts that every non-fit check and every existing tolerance is unchanged.
Keep the baseline directory if reproducing that historical comparison; the
controlled diagnostic and paired suite can also run independently.

Twenty unique public/synthetic models are fitted twice in the same R process.
Variants retain the original cs penalty, swap only its two null weights, apply
the explicit canonical directions, use Rust's captured penalty, or additionally
fix lambda to the Rust result. The last model's sp is an imposed input, not an
optimizer estimate. Original/canonical predictions, uncertainty, REML, edf,
variance, coefficients and covariance are retained. These diagnostics **never
replace original R targets** or change strict pass/fail statuses. Baseline/final
full captures provide an additional independent-process repeat check.

`gam-canonical-golden.json` contains public MinION selected-population aggregate
moments and actual controlled-R predictions, without read objects or metadata.
Its test checks agreement with the named convention **and disagreement with the
original reference endpoint**. It is not an unmodified-MinIONQC golden.


### Null-space sensitivity

Reproduce the remaining null-space sensitivity using the same public MinION file
copied by the paired suite (existing pinned R installation, no new environment):

```sh
/path/to/Rscript tests/reference/cs_sensitivity.R \
  benchmark-results/gam-paired/minion/inputs/source_001/sequencing_summary.txt \
  7 benchmark-results/cs-sensitivity.json
```

This changes only test knot coordinates, never the normal reference run. It shows
that R itself can swap .1/.01 null directions after an approximately 1e-15-hour
perturbation. The canonical Rust convention is deterministic; mismatches with the
original R result remain failures. No sensitivity result is used to widen target
tolerances or declare issue #12 complete. The committed validation summary retains
these measurements and the original-reference discrepancy counts.

## Independent-backend stability assessment

The 2026-10-23 [assessment](gam-final-assessment.json) preserves the original
18-case OpenBLAS baseline and adds a genuinely separate Netlib backend. Both
strict suites returned 1; no original target was replaced. Large diagnostic
matrices remain ignored. The native solver has no R/BLAS runtime dependency.

Use a **new** ignored directory. `--snapshot` refuses to overwrite its manifest,
checks the clean pinned upstream commit/source hash, and copies protected files.
The final assessor verifies original check-array signatures against committed
`revalidation-summary.json`; a retained full baseline is optional.

```sh
gam_run="$PWD/benchmark-results/gam-stability-repeat"
python3 tests/reference/assess_stability.py --snapshot "$gam_run" /tmp/minionqc-reference
export R_HOME=/path/to/pinned/lib/R
export PG_R_BINARY="$R_HOME/bin/exec/R"
python3 tests/minionqc_differential.py /tmp/minionqc-reference target/release/ontrunqc \
  --execute-r --capture-only --rscript tests/reference/r_backend.py \
  --output "$gam_run/paired-verified"
python3 tests/reference/check_revalidation.py "$gam_run/paired-verified/comparison.json"
```

The strict command is expected to **fail parity with exit 1**, not pass. Check
that all 18 R cases actually completed; dependency/execution errors are not
evidence. The separate revalidation checker passes only on identical original
numerical records, tolerances and classifications. `--capture-only` executes
original plot calculations and captures uncensored layers, but skips PDFs.

For the measured macOS arm64 second backend, obtain these exact conda-forge
packages without installing them over the R environment:

| Package | SHA-256 |
| --- | --- |
| [libblas 3.12.0 `2_h41cbde3_netlib`](https://api.anaconda.org/download/conda-forge/libblas/3.12.0/osx-arm64/libblas-3.12.0-2_h41cbde3_netlib.conda) | `dfdaa14b0f58fd367c6e5fb2e06d2ed33470fca00ca5fcbf91b5cba8debb6148` |
| [liblapack 3.12.0 `2_h41cbde3_netlib`](https://api.anaconda.org/download/conda-forge/liblapack/3.12.0/osx-arm64/liblapack-3.12.0-2_h41cbde3_netlib.conda) | `32cc603fa689b8657e4ed4beda5674eb49a04f8a0cefd2cdbf250d6c137703da` |

After downloading to `$gam_run/blas.conda` and `$gam_run/lapack.conda`, verify
those checksums **before** extracting. A `.conda` file is a ZIP containing
`pkg-*.tar.zst` and `info-*.tar.zst`; use `unzip`, `zstd` and `tar`:

```sh
mkdir -p "$gam_run/netlib/blas-info" "$gam_run/netlib/lapack-info"
unzip -p "$gam_run/blas.conda" 'pkg-*.tar.zst' | zstd -dc | tar -x -C "$gam_run/netlib"
unzip -p "$gam_run/lapack.conda" 'pkg-*.tar.zst' | zstd -dc | tar -x -C "$gam_run/netlib"
unzip -p "$gam_run/blas.conda" 'info-*.tar.zst' | zstd -dc | tar -x -C "$gam_run/netlib/blas-info"
unzip -p "$gam_run/lapack.conda" 'info-*.tar.zst' | zstd -dc | tar -x -C "$gam_run/netlib/lapack-info"
clang -dynamiclib -O2 tests/reference/backend_probe.c -o "$gam_run/backend_probe.dylib"
export PG_BACKEND_PROBE="$gam_run/backend_probe.dylib"
```

These binaries need compatible libgfortran 5/quadmath already provided by the
measured pinned installation. The recipe records clang 17/gfortran 13, SDK 11.0,
shared CMake and `-fno-optimize-sibling-calls`; full expanded build flags are
unavailable. The probe was compiled with Apple clang 21, `-O2` (no fast-math).
Do not call another platform/build equivalent without verifying it. Linux can
compile the probe with `cc -shared -fPIC -O2 ... -ldl -lm`; Linux backend
switching/cross-platform native stability were **not** executed in this assessment.

Run the original backend **first** to freeze its actual penalty bytes. Subsequent
backends read that file without overwriting it. Both sets use the identical
original observations, original-reference knots and native comparator:

```sh
tests/reference/r_backend.py tests/reference/gam_stability.R \
  "$gam_run/paired-verified" "$gam_run/stability-openblas-final.json" "$gam_run/frozen.rds"
tests/reference/r_backend.py tests/reference/gam_stability.R \
  "$gam_run/paired-verified" "$gam_run/stability-openblas-repeat.json" "$gam_run/frozen.rds"
tests/reference/r_backend.py tests/reference/gam_assessment.R \
  "$gam_run/paired-verified" "$gam_run/controlled-openblas.json"
tests/reference/r_backend.py tests/reference/cs_sensitivity.R \
  "$gam_run/paired-verified/minion/inputs/source_001/sequencing_summary.txt" \
  7 "$gam_run/cs-sensitivity-openblas.json"

export PG_BLAS_DIRECTORY="$gam_run/netlib/lib"
export PG_DYLD_PRINT_LIBRARIES=1
python3 tests/minionqc_differential.py /tmp/minionqc-reference target/release/ontrunqc \
  --execute-r --capture-only --rscript tests/reference/r_backend.py \
  --output "$gam_run/netlib-paired"
tests/reference/r_backend.py tests/reference/gam_stability.R \
  "$gam_run/paired-verified" "$gam_run/stability-netlib.json" "$gam_run/frozen.rds"
tests/reference/r_backend.py tests/reference/gam_stability.R \
  "$gam_run/paired-verified" "$gam_run/stability-netlib-repeat.json" "$gam_run/frozen.rds"
tests/reference/r_backend.py tests/reference/gam_assessment.R \
  "$gam_run/paired-verified" "$gam_run/controlled-netlib.json"
unset PG_BLAS_DIRECTORY PG_DYLD_PRINT_LIBRARIES
python3 tests/reference/assess_stability.py "$gam_run" "$gam_run/assessment.json"
```

The second strict command also returns 1. The summary's successful execution
means **evidence was checked**, not strict parity passed. It fails closed on
modified protected files, altered original comparisons, nonfinite coordinates,
different-sized arrays, changed primary failure sets, non-repeating model
payloads or BLAS aliases. Independent numerical outputs may legitimately change
exact/tolerant and intermediate counts; these remain separate from original R.

`r_backend.py` launches R's actual executable without its shell wrapper. On macOS,
protected shells strip DYLD variables, so exporting them before ordinary Rscript
did not switch the backend in our initial check. The launcher sets the dependency
path immediately before exec; no R installation symlink is changed. Verify actual
loaded-image inventories **and** BLAS/LAPACK symbol-provider paths/hashes from
`*.loaded.tsv`, never just a requested backend name. Whole-session inventories,
runtime environment, full matrix diagnostics, parameter values and original
suite logs remain in the experimental directory. Only bounded summaries belong
in Git. CPU model inspection was sandbox-denied on the measured host.

`gam_stability.R` reuses the unchanged `gam_audit` and `model_values` definitions.
Original/canonical/shared variants are explicitly diagnostic models. The assessor
uses the **actual model's effective penalty**, transformed to common nodal
coordinates; `gam_audit$shrink_penalty` reconstructs original cs and must not be
mistaken for a controlled model's penalty. Nine models, 192 knot fits and 36
matrix-only perturbations per backend test null rank/projectors, weighted
penalties, REML, coefficients, posterior covariance and uncensored 80-point
predictions. Independent processes repeat the full numerical payload. Same-penalty
controls isolate optimizer/covariance roundoff from changed regularization.
This investigation does not replace original-R targets or waive #12 acceptance.

## LOESS reference and notice

Regenerate synthetic LOESS goldens using:

```sh
/path/to/Rscript tests/reference/loess_reference.R tests/reference/loess-golden.json
```

These cover irregular/tied times, 3/4 observations with nonfinite bands, insufficient
or constant times, and the 999-observation method boundary. Rust checks 5,280
finite fitted/SE/bound values, unavailable states and the intermediate scale/df/
critical value. `loess_diagnostics` reports those scalar comparisons separately
from `gam_diagnostics`. Normal analysis still needs no R.

The LOESS delta-calibration constants in `trends.rs` follow `stats/src/loessf.f`
(ehg141/176), verified against the installed pinned R rather than substituting
exact hat-matrix residual df for R's default approximation. Original notice:

> Copyright (C) 1998--2020 The R Core Team.
> The authors of this software are Cleveland, Grosse, and Shyu.
> Copyright (c) 1989, 1992 by AT&T.
> Permission to use, copy, modify, and distribute this software for any
> purpose without fee is hereby granted, provided that this entire notice
> is included in all copies of any software which is or includes a copy
> or modification of this software and in all copies of the supporting
> documentation for such software.
> THIS SOFTWARE IS BEING PROVIDED "AS IS", WITHOUT ANY EXPRESS OR IMPLIED
> WARRANTY. IN PARTICULAR, NEITHER THE AUTHORS NOR AT&T MAKE ANY
> REPRESENTATION OR WARRANTY OF ANY KIND CONCERNING THE MERCHANTABILITY
> OF THIS SOFTWARE OR ITS FITNESS FOR ANY PARTICULAR PURPOSE.

## Input-integrity repetition (2026-10-19)

All 18 cases were executed again with the same installed pinned R environment
using `--capture-only`; all primary check records and both diagnostic arrays
were exactly equal to the retained `parity-final/comparison.json`. Original
`validation-summary.json` and all tolerances/targets remain unchanged. The new
small `revalidation-summary.json` records each array's canonical SHA-256, source
and comparison hashes, versions and unchanged status counts. It contains no
individual sequencing records.

```sh
python3 tests/reference/check_revalidation.py benchmark-results/input-integrity/paired-r/comparison.json
```

This checker fails on any changed value/tolerance/classification, incomplete
case or environment/version change. Success means **unchanged evidence**, not
passing GAM parity: the original suite still exits 1 with 1,542 primary GAM
failures and 1,912 intermediate failures. New reference-harness unit tests verify
that it never silently accepts a changed value, tolerance, status or incomplete
execution. Exact check-array signatures are a same-environment regression
contract; other BLAS environments are not automatically accepted.


## Reference histogram and Bin2D suite (#19)

The existing plot audit accepts `--reference-bins` to enable the production
`--minionqc-plot-bins` model in its file-scoped validation probe. Original R
scripts, input bytes, grouping and cutoffs are unchanged. Capture still retains
original layers; no corrected Rust result replaces an R target.

```sh
Rscript tests/reference/bin_layers.R tests/reference/bin-layers-golden.json
cargo test --locked plot_bins::tests
python3 tests/reference/plot_audit.py /path/to/pinned/minion_qc \
  --rscript /path/to/Rscript --reference-bins \
  --output benchmark-results/bin-audit --gallery benchmark-results/bin-gallery
```

The synthetic generator verifies installed pinned versions, records upstream
empty/degenerate failures, and emits bounded edges/counts plus four corrected
channel-count-control families. Rust tests consume the committed goldens without
R. Public comparisons check every corresponding bin/cell (integer counts exact;
coordinates `1e-12 + 1e-13*abs(reference)`). The true-count exception requires
independent membership checks against raw public channel observations for both
native n and reference n(n+1)/2; an unexplained mismatch fails.

See [executed validation](bin-validation-summary.json) and
[scientific interpretation](../../docs/minionqc-comparison.md#reference-histogram-and-bin2d-validation-19).
Original smoothing/KDE goldens and the 1,542 primary / 1,912 intermediate GAM
failure classifications remain independent. `--reuse` may reuse hash-identical
R captures but always recomputes conclusions; omit it for a fresh R execution.

## Reference temporal suite (#20)

Use the existing pinned environment; normal analysis still needs no R:

```sh
Rscript tests/reference/temporal_layers.R /path/to/pinned/minion_qc/MinIONQC.R \
  tests/reference/temporal-layers-golden.json
cargo test --locked temporal::tests
python3 tests/reference/plot_audit.py /path/to/pinned/minion_qc \
  --rscript /path/to/Rscript --reference-bins --reference-time \
  --output benchmark-results/time-audit --gallery benchmark-results/time-gallery
python3 tests/reference/temporal_benchmark.py --records 1000000
```

The synthetic generator evaluates the **original parsed** `add_cols()` and
single-flowcell hourly/line expressions, with the actual R9.5 map. Its eight
cases cover second/hour boundaries, missing hours, MinION/PromethION tied times,
zero lengths, signed Q, sparse long runs and singleton groups. Rust consumes
these committed fictional goldens without requiring R. A separate regression
ensures the audit never replaces a vertical tied jump with a diagonal.

`--reference-time` executes native file-scoped probes first, then provides their
bounded curves as **queries** to the unchanged R plotting layers. The R capture
evaluates every original timestamp, plus actual GeomLine left/right limits and
vertical ranges. Hour pairs, populations, endpoint coordinates and integer
terminal yields have zero tolerance. Only GB-to-bp floating arithmetic permits
`1e-5 bp` roundoff; no counting/time or GAM tolerance is widened. Per-group
maximum errors, number of input/rendered/omitted points and explicit yield/tie
bounds are separate from bounded 256-point residual samples used in the gallery.

A simplified curve is never classified as original point-for-point parity.
Shared-time cumulative maxima are invariant; R's row-order-sensitive internal
paths are reported as a separate tied-order difference. Empty sources/empty Q
populations render safely in ONTRunQC; original R exits on an empty population.
Reference-mode data remain additive, independent of histogram/fitting/density
options and canonical 15-minute diagnostics.

The optional resource tool generates independent fictional sources (one plain,
one gzip), reuses child-specific CPU/RSS and sampled scratch accounting, checks
canonical JSON and worker determinism, and records numeric-sort and rendering
phase logs. Generation precedes measurement; run after other builds/audits stop.

## Yield-by-length validation (#21)

The existing plot audit now writes `yield-validation.json` alongside `audit.json`.
It captures every finite original R tied-length minimum/maximum against an
independent complete length-frequency table, the original crop/clipped counts,
terminals, native envelope deviations, and separate original per-read/tie-path
differences. Exact integer reference checks have zero tolerance; only display
interpolation/GB conversion permits 1e-5 bp roundoff. No GAM tolerance changes.

```sh
python3 tests/reference/plot_audit.py /path/to/pinned/minion_qc \
  --rscript /path/to/pinned/Rscript --reference-bins --reference-time \
  --output benchmark-results/yield-audit --gallery benchmark-results/yield-gallery
Rscript tests/reference/yield_layers.R /path/to/pinned/MinIONQC.R \
  tests/reference/yield-layers-golden.json
```

`--reuse` requires matching source/input/capture/candidate hashes, then recomputes
conclusions and images. Validation-only exact frequency tables stay outside Git;
production retains only bounded display vertices. The committed
[yield summary](yield-validation-summary.json) includes actual R evidence,
independent full-report checks and the optional before/after benchmark.
Use [yield_benchmark.py](yield_benchmark.py) with separately built baseline
`876bf65`; [commands/measurements](../../docs/benchmark.md#yield-by-length-display-21)
describe input generation, source passes, sampling and output-size trade-offs.
This suite does not rerun or waive the strict 18-case GAM comparison.
