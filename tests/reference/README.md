# Optional executable MinIONQC reference

The normal Rust build never needs R. Use the clean upstream checkout at commit
`54485d586038d8ee36f168dd94f31c423f52f581` (v1.4.1), including its public fixtures:

```sh
git clone https://github.com/roblanf/minion_qc /tmp/minionqc-reference
git -C /tmp/minionqc-reference checkout 54485d586038d8ee36f168dd94f31c423f52f581
cargo build --release --locked
python3 tests/minionqc_reference.py /tmp/minionqc-reference --execute-r \
  --rscript /path/to/Rscript --output benchmark-results/minionqc-r
```

Use an existing R installation; no environment is created by this command. There
is no automatic R CI build. The optional Dockerfile remains an alternative for
users who explicitly want a container: build it with `docker build -t
poregauge-minionqc-r -f tests/reference/Dockerfile tests/reference`, then replace
`--rscript /path/to/Rscript` with `--docker poregauge-minionqc-r`.

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

This runs the unmodified script on unchanged source bytes, with matching Q
cutoffs, one processor, explicit 8-hour mux references and PDF figures. The `ggsave` wrapper also calls
`ggplot_build()` and records numerical layers, method/formula, panels, legends
and axis labels. `--capture-only` skips figure rendering but still runs upstream
summary/plot calculations. `--case short` selects a case. Full runs include
MinION, PromethION, two MinION inputs/combined QC, short, 1D2, RNA and synthetic
zero/cutoff/N50/ultralong edge cases, small/large/irregular/sparse fits and
multimodal/skewed/constant/near-constant/outlier densities and singleton groups
with unequal source sizes (18 cases). Nothing uses private sequencing data.

Each case saves original R YAML/PDF, capture JSON, PoreGauge JSON/YAML, plot
inventory and logs. `comparison.json` includes source/input hashes, Q cutoffs,
reference values, differences, tolerances and six explicit status categories.
Unknown mismatches are defects requiring investigation, never accepted silently.
Counts/bases use exact comparisons; one-decimal summaries allow 0.050000001;
serialized Gb yields allow 50.1 bp. Zero-denominator/N50 tie differences are named;
the upstream ultralong endpoint bug requires a numerical proof before classification.
Runs return 1 for suspected PoreGauge defects, 2 for unverified execution, 0 only
when every requested comparison is accounted for. Missing R never generates
invented reference values. R's `quit()` or partial output may prevent comparisons;
inspect logs and retain Not yet verified until the cause is established.

Public files can contain several run IDs. MinIONQC groups by parent directory;
PoreGauge correctly groups by run ID. Pooled statistics are compared on identical
file populations; source/file versus instrument identity is not silently equated.
Saved upstream YAML remains a separate fast independent check (omit `--execute-r`).
Harness logic tests: `python3 -m unittest discover -s tests/reference`.

The ignored Rust `reference_probe::export_report` test reuses the actual parser and
aggregator on the upstream file-scoped population for channel/curve comparisons.
This is necessary because upstream drops run IDs before analysis. Normal CLI
reports are also generated and checked; production run grouping is never changed.
Probe artifacts are validation-only and cannot be mistaken for instrument metadata.

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
tolerance; unsupported LOESS intervals remain explicitly Not yet verified. Density
integrals are over the plotted domain; unplotted Gaussian tails are not silently
renormalized to force area one. Constant-domain evaluation is at the unchanged
single coordinate. A `predictdf()` exit trace also captures uncensored predictions,
observation counts, GAM knots, smoothing parameters and residual variance before
plot-axis clipping. Comparisons use these predictions when available, including
negative fitted values hidden by an axis limit. Nonfinite reference predictions
remain Not yet verified; null is not numerical evidence.
Failed point/method comparisons are never automatically accepted.

`trends-golden.json` is a small, synthetic-only numerical snapshot of the actual
`predictdf`/`StatSmooth` values from `fit-small`, `fit-large`, `zero-cutoff`,
`n50-tie` and `ultralong-endpoint`, including
their exact TSV bytes, reference versions and capture SHA-256. The Rust unit test
re-runs parsing/fitting and compares all 1,598 finite fitted values and 960
large-case GAM uncertainty values at the same tolerance. It is not evidence
that the other paired cases pass. The latest paired run is recorded in
`validation-summary.json`; the full ignored report is
`benchmark-results/gam-paired/comparison.json`. Its exit 1 reflects remaining
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
