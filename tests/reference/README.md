# Optional executable MinIONQC reference

The optional **MinIONQC reference** GitHub Actions workflow runs the same command
on an Ubuntu runner, builds the pinned R container and retains actual reference
reports, numeric captures, logs and comparisons as a 14-day artifact, including
on failure. It runs manually or on explicitly named `scientific-parity/**`
branches; ordinary pushes and pull requests do not require R. An unsuccessful
reference comparison must be investigated, not treated as a passing CI result.

The normal Rust build never needs R. Use the clean upstream checkout at commit
`54485d586038d8ee36f168dd94f31c423f52f581` (v1.4.1), including its public fixtures:

```sh
git clone https://github.com/roblanf/minion_qc /tmp/minionqc-reference
git -C /tmp/minionqc-reference checkout 54485d586038d8ee36f168dd94f31c423f52f581
cargo build --release --locked
docker build -t poregauge-minionqc-r -f tests/reference/Dockerfile tests/reference
python3 tests/minionqc_reference.py /tmp/minionqc-reference --execute-r \
  --docker poregauge-minionqc-r --output benchmark-results/minionqc-r
```

The reference environment is R **4.4.1**, ggplot2 **3.5.1**, mgcv **1.9-1**, with
transitive R dependencies from the **2024-07-01** CRAN snapshot. These versions
are an explicitly selected reproducible environment, not a claim about the
unrecorded package versions originally used by MinIONQC's author. The image tag
and OS package repositories are not bit-for-bit immutable: record the resolved
Docker image ID/digest alongside the capture's full package versions/sessionInfo
when archiving results. Native execution supports `--rscript /path/to/Rscript`
with the same checked versions. Locale/timezone are fixed in the image; seed is 1.

This runs the unmodified script on unchanged source bytes, with matching Q
cutoffs, one processor, explicit 8-hour mux references and PDF figures. The `ggsave` wrapper also calls
`ggplot_build()` and records numerical layers, method/formula, panels, legends
and axis labels. `--capture-only` skips figure rendering but still runs upstream
summary/plot calculations. `--case short` selects a case. Full runs include
MinION, PromethION, two MinION inputs/combined QC, short, 1D2, RNA and synthetic
zero/cutoff/N50/ultralong edge cases, small/large/irregular/sparse fits and
multimodal/skewed/constant/near-constant/outlier densities with unequal source sizes. Nothing uses private sequencing data.

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
one grid interval. These are **provisional targets, not measured R accuracy**.
Unsupported confidence intervals remain explicitly Not yet verified. Density
integrals are over the plotted domain; unplotted Gaussian tails are not silently
renormalized to force area one. Constant-domain scaling remains pending actual R
verification. Failed point/method comparisons are never automatically accepted.
