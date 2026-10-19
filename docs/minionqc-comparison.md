# MinIONQC v1.4.1 vs ONTRunQC — detailed compatibility comparison

**Status:** source-backed comparison draft, 2026-10-19. **Product priority:** MinIONQC compatibility before ONTRunQC-only extensions.  
**Reference:** [MinIONQC v1.4.1, pinned R script](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R) (commit 54485d586038d8ee36f168dd94f31c423f52f581).  
**ONTRunQC reference baseline:** [repository](https://github.com/PhoenixEmik/ONTRunQC), renamed from PoreGauge; main at 6f8b51d635db when this comparison was drafted. Scientific differential baseline is older and **has not been re-executed for this documentation commit**.  
**Pinned reference software for actual paired checks:** R 4.4.1, ggplot2 3.5.1, mgcv 1.9-1 on the documented macOS ARM64 environment.  
**Sources of detailed verification:** [engineering parity ledger](minionqc-parity.md), [metric definitions](metrics.md), [reference harness](../tests/reference/README.md), [validation summary](../tests/reference/validation-summary.json), and [unchanged revalidation](../tests/reference/revalidation-summary.json).

## 1. Executive summary and meaning of "parity"

MinIONQC is an R tool for assessing Oxford Nanopore sequencing-summary files and generating static per-flowcell and combined plots. ONTRunQC is an independent Rust CLI analyzing sequencing-summary TSV with exact core statistics, self-contained offline HTML/JSON, optional YAML and independent figures, with run-aware multiple-input analysis. **Neither is a general FASTQ/BAM/POD5 QC engine as implemented here.**

Functional coverage of the pinned MinIONQC report families is broad, but **strict numerical, plot-bin, pixel-rendering and exact output-default parity are not claimed**. In particular:

- All 18 reference cases completed their original pinned R analysis. The strict paired suite **exits 1**: 1,542 original-R GAM fitted/uncertainty checks remain outside unchanged tolerance, and 1,912 separate GAM intermediate comparisons differ. No R check has been silently reclassified.
- Measured primary statuses in the retained run: **65,118 exact**, **75,241 within tolerance**, **36 intentional semantic differences**, **8,954 checks classified as proven upstream defects**, **1,542 suspected ONTRunQC GAM differences**, and **0 unverified in that particular paired suite**. These are *check counts*, not distinct bugs or proof that every possible input was tested.
- The 1,542 failing checks are 390 fitted values, 381 standard errors, 381 lower and 390 upper confidence bounds, for the same MinION Q-selected models repeated in single-run and combined plot contexts.
- Independent R-grid KDE checks passed for 52 curves / 26,624 point pairs; LOESS pointwise values and supported intervals passed their recorded reference checks. Core population/threshold/pooled analyses have their own independent test evidence.
- The compiled/documented Rust GAM uses an explicit, stable cubic-shrinkage null-space convention. It is *not* the same penalty orientation that pinned mgcv selects for every input; an R-only controlled experiment reproduces the mismatch after changing only those shrinkage directions. A documented explanation is **not** strict numerical equivalence.

### Evidence/status vocabulary

| Status | Meaning | How this document uses it |
| --- | --- | --- |
| Exact / verified | Matched actual pinned R values or an independent exact calculation under stated definitions | Cite fixture/test and comparison scope |
| Functional with differences | Provides the same scientific question/output family but with different binning, sampling/raster, formatting, population selection or default | Identify the precise difference |
| Intentional semantic difference | Native definition differs intentionally or corrects a separately proved reference defect | Do not present as identical R output |
| Strict numerical failure | Values differ beyond existing pinned R numerical tolerance | Keep visible and refer to #12 |
| Unverified | Evidence has not been executed or property not established | Do not infer success from routine Rust CI |

**Primary unresolved acceptance:** [#8 multi-run combined parity](https://github.com/PhoenixEmik/ONTRunQC/issues/8) retains its original strict fitted-curve requirement; [#12 GAM smoothing](https://github.com/PhoenixEmik/ONTRunQC/issues/12) retains the actual failing R comparison. A proposed split between functional and strict gates is not an approved closure or waiver.

## 2. Input compatibility and population definitions

| Topic | Pinned MinIONQC | ONTRunQC | Consequence |
| --- | --- | --- | --- |
| Input | Plain sequencing_summary.txt, single file or recursive directory | Plain uncompressed, unquoted TSV; explicit files or recursive matching sequencing_summary*.txt/.tsv | Direct gzip reading is **not** yet supported by ONTRunQC; postponed [#17](https://github.com/PhoenixEmik/ONTRunQC/issues/17) is outside MinIONQC-first work |
| Modern headers | Reads compatible template-read fields | Validates required modern fields; accepts auto/legacy modes | Invalid required data cause errors, not silent drops |
| Legacy / 2D / 1D2 | Historical Albacore/Guppy handling, complete 2D field substitutions | Explicit template/2D choice, no per-row template fallback; absent modern fields stay unavailable | Public legacy and 1D2 comparisons exist |
| Direct RNA | Removes rows identified by YHR174W calibration strand | Exact named control exclusion with excluded-record count | Exclusions must not be mistaken for a general RNA QC filter |
| Original grouping | File and parent-directory-derived flowcell labels | Actual run-ID grouping across chunks; unknown identities kept explicitly separate | Run is not assumed to equal a distinct physical flowcell |
| Native pass/fail | Core upstream plot split is based on mean Q threshold | Native passing uses only passes_filtering when available | Native pass and Q-selected subsets are *not interchangeable* |
| Q-selected | Reads with per-read mean Q ≥ configured cutoff | Separate inclusive Q-selected population, default Q 7 | Selected stats preserve independent Q threshold |
| Zero-length records | "All reads" plots/statistics can include summary rows with length zero | Total summary rows are kept separately; basecalled length/Q metrics use positive-length reads | Denominator differences affect means/medians and some plot populations |
| Malformed values | readr/reference behavior may drop problem rows | Invalid required rows fail with source/line context | Intentional correctness/diagnostic policy |
| Device geometry | Reference uses observed max channel to choose MinION vs PromethION | Explicit MinION R9.5 512 or PromethION 3000 layout | No potentially wrong inferred instrument shape |
| Processing | R mclapply across summary files | Bounded parallel *independent run components* with ordered outputs and exact pooled reduction | Shared run-ID files never silently process as distinct runs |

See [the original loader and transformations](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L159-L277) and [ONTRunQC input parser](../src/input.rs), [batch grouping](../src/batch.rs) and [integrity checks](../src/integrity.rs).

## 3. Scientific statistics: values, definitions, edge cases

| Statistic / family | MinIONQC meaning | ONTRunQC meaning | Parity / evidence |
| --- | --- | --- | --- |
| Total reads and yield | Reference "All reads" summary from TSV populations | Distinguishes summary records, positive-length basecalled reads, and all yield in integer bp | Exact populations compared; labels/denominators require attention |
| Native passing | Not the same as a historical mean-Q cutoff | Optional literal passes_filtering flag; missing metadata stays unavailable | Additional ONTRunQC analysis, never inferred from Q |
| Q≥cutoff count/yield | Reference Q-selection and plotted All/Q categories | Independent inclusive per-read mean-Q cutoff; exact count and integer bases | Actual reference and synthetic cutoff-boundary tests |
| Mean/median length | Reference can count length-zero summary rows; rounded YAML | Exact selected positive-length observations; full precision | Not necessarily the same denominator |
| Mean/median Q | Reference summary selection/rounding | Exact arithmetic per-read reported Q values within the selected population | Compare with original selection and displayed precision |
| N50 | Reference strict-half boundary behavior in certain cases | Cumulative yield reaching ≥ half total, with exact ties handled | Intentional strict-half edge difference; standard cases match |
| Long-read thresholds | Labels >10,20,50,100,200,500 kb and >1 Mb, but underlying selection inclusive | Exact integer counts/bases at inclusive thresholds | Seven lengths tested, including boundaries |
| Ultralong subset | Longest-first prefix constrained by reference N50 ≥100 kb convention | Exact longest-first prefix with reference strict-half convention and corrected endpoint | Known original endpoint bug classified explicitly |
| Channel counts/yield | Four channel summary families in R; a count expression has a known defect | True counts, bases, mean/median length, distinct populations and explicit bins | Scientific correction; not byte-equivalent to bug |
| Time-dependent yield | Per-read cumulative yield ordered by read start | 15-minute bins; cumulative absolute yield at bin ends | Same overall yield, different temporal resolution |
| Reads per hour | Actual integer-hour read counts, zero-filled missing hours | 15-minute read counts, explicitly labeled | No identical hourly chart data claim |
| Read-length vs quality | MinION read-level scatter; PromethION bin2d | Exact small-run points; bounded full-count MinION raster; fixed-bin PromethION heatmap | All eligible positive-length reads contribute; pixel/color behavior differs |
| Sequencing speed proxy | Not full instrument occupancy or telemetric throughput | Reported template length/duration when available | ONTRunQC addition, not a direct MinIONQC equivalence claim |

Original metrics source: [summary.stats / channel.summary](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L347-L420). Detailed equations, zero-length and N50 examples: [ONTRunQC metrics](metrics.md).

**Public pooled multi-run reference:** 608,208 summary records; 606,143 positive-length basecalled reads; 7,203,301,321 bases; N50 = 33,161 bp; median = 2,616 bp. This is the previously measured reference case, **not** a rerun during this documentation update.

## 4. One-to-one scientific plot inventory

**Important:** a corresponding plot/report family is not a claim of identical pixel layouts or ggplot-generated bin boundaries. All/selected Q populations, native filtering and zero-length eligibility may require different axes/labels.

| Original MinIONQC output (single-run) | Original layer/semantics | ONTRunQC equivalent | Difference / verification |
| --- | --- | --- | --- |
| length_histogram | Two All/Q panels, 300 ggplot bins after length transform | Read-length histograms (count/yield) and independent figures | Fixed, documented bins; count/yield conserved, not pixel-identical |
| q_histogram | Mean-read-Q distribution by All/Q | Quality histograms (count/yield) | Fixed bins and full-precision totals; rounded display differs |
| flowcell_overview (MinION only) | Faceted read start-time vs log length, colored by Q | Full-data MinION R9.5 physical overview | Exact points for small inputs; all-count 64×40 pixels/channel for dense inputs + extrema; explicitly not point-for-point ggplot |
| yield_over_time | Per-read cumulative gigabases vs time | Yield-over-time cumulative series | ONTRunQC 15-minute end bins, not original read-level path |
| yield_by_length | Yield retained above minimum read length, cropped display X | Exact retained-yield queries and graph | ONTRunQC shows broader range, exact integer underlying queries |
| length_by_hour | ggplot geom_smooth of per-read length vs start-time | Optional fitted length trends and unsmoothed observed means | LOESS verified; GAM original-R null-space difference persists (#12) |
| q_by_hour | ggplot geom_smooth of per-read Q vs start-time | Optional Q-score fitted trends and observed means | Same remaining GAM issue; do not substitute binned means for R raw fit |
| reads_per_hour | Counts in integer-hour bins; missing hours zero-filled | Start-time interval read-count chart | ONTRunQC 15-minute bins, different time support |
| length_vs_q | MinION scatter (events/base color); PromethION bin2d | MinION exact/full-count scatter, PromethION count heatmap | Natural-log positive events/base meaning preserved; dense colors and alpha differ |
| channel_summary | Four per-channel distribution facets | Channel-distribution charts and exact tables | True read counts rather than reference count bug; bins differ |
| gb_per_channel_overview | Spatial channel yield tiles by All/Q | Explicit MinION/PromethION physical yield maps | Verified layouts; legend/colors/missing-vs-zero and optional native map differ |

Original single-run figures: [R single.flowcell, lines 423–723](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L423-L723). ONTRunQC charts are embedded in offline report.html, and requested independent figures in figures/ with manifest.json. Do not assume identical file basenames when cross-comparing.

| Original combined/per-source output | Reference meaning | ONTRunQC equivalent | Difference / status |
| --- | --- | --- | --- |
| combined_length_histogram | Pooled All/Q length distributions | Exact pooled histograms | Same pooled question, independent bins |
| combined_q_histogram | Pooled All/Q Q distributions | Exact pooled quality histograms | Same pooled question, independent bins |
| combined_yield_by_length | Pooled minimum-length yield | Exact pooled retained-yield chart | Underlying counts/bases exact; original hardcodes .png filename in one path |
| length_distributions | Per-flowcell log-read-length density | Optional shared R-grid KDE comparisons | 52 total measured density curves within declared tolerance |
| q_distributions | Per-flowcell Q density | Optional R-grid Q density | Same R-grid evidence, distinct summary-zero-Q population handling |
| yield_over_time (multi) | Per-flowcell cumulative yield | Per-run comparison timeline | Different plotting intervals/labels, conserved totals |
| yield_by_length (multi) | Per-flowcell retained bases above length | Per-run yield-by-length comparisons | Exact thresholds/bases; display range differs |
| length_by_hour (multi) | Per-flowcell ggplot smoothed read length | Optional per-run fitted/observed trends | GAM remains partial as above |
| q_by_hour (multi) | Per-flowcell ggplot smoothed Q | Optional per-run fitted/observed trends | GAM remains partial as above |
| combined summary.yaml | All/Q combined statistics | Optional ONTRunQC summary.yaml plus JSON | Source labels, exact integer precision, denominators and known fixes differ |

Original combined figures: [R combined.flowcell](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L725-L805), [multi.plots](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/MinIONQC.R#L817-L891).

## 5. Command-line, defaults and output behavior

| Original option | Original MinIONQC | ONTRunQC | Difference |
| --- | --- | --- | --- |
| -i, --input | Mandatory file/directory | Positional file/directory or repeated --input | ONTRunQC requires analyze subcommand |
| -o, --outputdirectory | Optional, can write next to input | --output normally required; --minionqc-compat offers fresh safe ontrunqc_qc directory | Avoids silently overwriting input files |
| -q, --qscore_cutoff | Default 7, numeric | Default 7, inclusive finite signed Q cutoff | Modern schema Q itself must remain valid/nonnegative |
| -p, --processors | Default 1; R mclapply across files | Bounded independent-component workers | Not fork-equivalent; grouping integrity prioritized over naive file dispatch |
| -s, --smallfigures | TRUE/FALSE | Bare flag and TRUE/FALSE forms | Actual physical sizing is independently defined |
| -c, --combined-only | TRUE/FALSE | Bare flag and TRUE/FALSE forms | Combined-only skips individual run outputs |
| -f, --format | PNG default, PNG/PDF/PS/JPEG/TIFF/BMP | HTML/JSON default, SVG and all six static formats on normal build | One requested independent format per execution; no bit/pixel identity |
| -m, --muxes | Reference grid; 0 uses historical single-run interval, combined default 8h | 0 disables normally; historical grid with --minionqc-compat | Generated markers are not measured MinKNOW events |
| -a, --add-stat | TRUE/FALSE | Bare flag and TRUE/FALSE forms | Annotation style/placement differs |

### Example: single-run comparison

    Rscript MinIONQC.R -i sequencing_summary.txt -o minionqc_output -q 9 -f png

    ontrunqc analyze sequencing_summary.txt --output ontrunqc_output --qscore-cutoff 9 --format png --yaml --minionqc-trends

If the comparison includes multi-run R-grid density layers, add --minionqc-density to ONTRunQC. Neither option makes every ONTRunQC default numerically identical to R.

### Example: multiple runs, combined only

    Rscript MinIONQC.R -i input_directory -o minionqc_output -c TRUE -p 2 -f pdf

    ontrunqc analyze input_directory --output ontrunqc_output --combined-only --processors 2 --format pdf --yaml --minionqc-density --minionqc-trends

Use --minionqc-compat only if the historical mux marker/output semantics are needed and understand that safe output placement intentionally differs.

### File formats

ONTRunQC always writes report.json and report.html; --yaml additionally writes summary.yaml. --format svg/png/pdf/ps/jpeg/tiff/bmp creates independent scientific figures. PNG/TIFF/BMP are lossless 300-DPI RGB; JPEG is lossy quality 95; PDF preserves vector text/graphics where possible; **PS is raster-backed Level 3 at 300 DPI**, not vector-equivalent to ggplot output. Minimal --no-default-features builds explicitly omit the six static conversions. See [static figure definitions](metrics.md#native-static-figure-exports-15).

## 6. Strict numeric GAM difference: measured rather than waived

Original-R comparison targets are still **absolute 1e-6 plus relative 1e-6** for fitted/uncertainty predictions. Recorded MinION Q≥7 original R vs native Rust maximum deviations:

| Compared value | Original pinned R vs ONTRunQC | Controlled R with canonical/swapped null directions vs ONTRunQC |
| --- | ---: | ---: |
| Length fitted mean | 1.715425619 bp | 2.155e-7 bp |
| Length standard error | 0.466714392 bp | 3.877e-10 bp |
| Length CI lower / upper | 2.630169018 / 0.800682220 bp | 2.163e-7 / 2.148e-7 bp |
| Q fitted mean | 8.974e-4 | 5.607e-12 |
| Q standard error | 2.880e-5 | 2.010e-14 |

The R reference consistently reproduces its original output in repeated runs on the pinned local installation. Tiny perturbations to knots can change sorting of two near-null eigen-directions in mgcv's cubic shrinkage spline; unequal 0.1 and 0.01 shrinkage weights then act on different directions. ONTRunQC uses a deterministic convention. Controlled R models with matching directions reproduce Rust predictions within the existing tolerance, but **the original reference penalty is genuinely different for the affected case**.

Do not call these original-R comparisons passing, globally relax tolerances, or relabel a controlled R fit as an unmodified MinIONQC reference run. Run and source hashes are in the [reference harness documentation](../tests/reference/README.md) and the [machine-readable evidence](../tests/reference/validation-summary.json). Cross-BLAS portability remains not verified.

## 7. Evidence reproduction and limitations

Routine code verification:

    cargo fmt --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked
    cargo build --release --locked
    cargo test --locked --no-default-features

Actual pinned R reference (requires installed pinned R environment and the external reference checkout):

    python3 tests/minionqc_reference.py /path/to/pinned/minion_qc --execute-r --rscript /path/to/Rscript --capture-only --output benchmark-results/current-r

A nonzero exit can be **expected and meaningful** when the strict GAM mismatches remain. To compare with historical evidence rather than falsely claim a pass:

    python3 tests/reference/check_revalidation.py benchmark-results/current-r/comparison.json

Further specialized tests: tests/minion_visualization.py for complete-data MinION pixel coverage; tests/static_exports.py for actual format decoding; tests/parallel_runs.py for deterministic combined results; tests/reference/gam_assessment.R and cs_sensitivity.R for controlled GAM diagnostics. See [benchmark documentation](benchmark.md) for verified environments and resource measurements.

**Limits not established by the cited checks:** exact ggplot pixels/palette/alpha, universal input/schema coverage, an independent second BLAS, live source mutation immunity on NFS, or full-run speed guarantees. Strict numeric status remains Partial. The current comparison consolidates previously recorded evidence rather than asserting a new live R benchmark.

## 8. What must happen next

1. **Documentation acceptance [#18](https://github.com/PhoenixEmik/ONTRunQC/issues/18):** verify every mapping and referenced source, review metrics/definitions and spot-check commands with the actual pinned reference. Link this detailed document from user-facing README and the engineering parity ledger.
2. **Combined scientific parity [#8](https://github.com/PhoenixEmik/ONTRunQC/issues/8):** preserve its original strict fitted-temporal requirements; do not mark closed based only on this documentation.
3. **Original GAM strict numeric parity [#12](https://github.com/PhoenixEmik/ONTRunQC/issues/12):** retain 1,542 failures and independently justified explanation; separate scientific functionality from full original-R numerical equality without silently changing gates.
4. **Later product extensions:** gzip [#17](https://github.com/PhoenixEmik/ONTRunQC/issues/17) and other new QC families are deferred until compatibility documentation and gates receive explicit attention.

This document is meant to be read as a **comparison and disclosure**, not a claim that every numerical output matches the original MinIONQC exactly.
