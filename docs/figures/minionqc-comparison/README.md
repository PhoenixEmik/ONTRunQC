# Actual MinIONQC / ONTRunQC comparison gallery

Original R smallfigures TRUE, PNG 60 DPI for audit readability; native canonical figures 300 DPI; gallery resized and palette-reduced to 256 colours only for documentation. Original files remain unchanged.

Each card uses identical original input bytes and Q cutoff. File-scoped validation reports match R's grouping; these are not instrument run-ID reports. C never treats different histogram grids as commensurable residuals. Full captures/reports stay in ignored `benchmark-results/plot-audit/`.

## minion · length_histogram · source_001

Counts on DIFFERENT grids: R 300 automatic log10 bins; ONTRunQC fixed bp intervals (centres shown). No binwise residual is defined.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![length_histogram comparison](minion-source_001-length_histogram.png)

## minion · q_histogram · source_001

Counts on DIFFERENT grids: R 300 automatic Q bins; ONTRunQC fixed Q intervals. Zero-length Q population differs.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![q_histogram comparison](minion-source_001-q_histogram.png)

## minion · flowcell_overview · source_001

C compares complete finite R panel counts against native positive-length counts/channel. R overview grid includes every finite XY row; native counts/raster conservation separately tested. A/B layouts are exact R9.5.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![flowcell_overview comparison](minion-source_001-flowcell_overview.png)

## minion · yield_over_time · source_001

R exact cumulative-at-read-start queried every 15 min; ONTRunQC bin-end values. Within-bin paths differ; terminal bp conserved.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![yield_over_time comparison](minion-source_001-yield_over_time.png)

## minion · yield_by_length · source_001

R cropped per-read cumulative path; ONTRunQC exact inclusive minimum-length queries. Ties, crop and bounded display are distinct.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![yield_by_length comparison](minion-source_001-yield_by_length.png)

## minion · length_by_hour · source_001

Actual original-R fitted layer vs native Rust curve; original strict GAM failures remain failures in the separate differential suite.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![length_by_hour comparison](minion-source_001-length_by_hour.png)

## minion · q_by_hour · source_001

Actual original-R fitted layer vs native Rust curve; original strict GAM failures remain failures in the separate differential suite.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![q_by_hour comparison](minion-source_001-q_by_hour.png)

## minion · reads_per_hour · source_001

R counts per integer hour; ONTRunQC counts per 15 minutes. Heights are NOT rates with equal exposure.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![reads_per_hour comparison](minion-source_001-reads_per_hour.png)

## minion · length_vs_q · source_001

C: complete 64-column log-length marginal from ALL finite R scatter coordinates versus coarsened native full-count pixels (or exact points). This is a diagnostic aggregation, not an original R histogram or independent point-by-point rendering.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![length_vs_q comparison](minion-source_001-length_vs_q.png)

## minion · channel_summary · source_001

Original read-count statistic is n(n+1)/2 (BUG); Rust uses true n. Both show channel frequency, with different bin boundaries.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![channel_summary comparison](minion-source_001-channel_summary.png)

## minion · gb_per_channel_overview · source_001

C compares exact yield/channel; source spatial cells and panel/population totals retained. A/B share verified mapping, different missing-cell styling and palette.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![gb_per_channel_overview comparison](minion-source_001-gb_per_channel_overview.png)

## promethion · length_vs_q · source_001

C sums each length column over Q: R automatic log10 bin2d versus fixed log2 length intervals. Different boundaries; A/B show actual 2D geometry. Native final overflow is plotted at its lower bound.
Q cutoff: 7; input SHA-256 prefixes: dd508d9dc645519e. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![length_vs_q comparison](promethion-source_001-length_vs_q.png)

## short · reads_per_hour · source_001

R counts per integer hour; ONTRunQC counts per 15 minutes. Heights are NOT rates with equal exposure.
Q cutoff: 8; input SHA-256 prefixes: 60a070e325b0c2e3. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![reads_per_hour comparison](short-source_001-reads_per_hour.png)

## 1d2 · length_vs_q · source_001

C: complete 64-column log-length marginal from ALL finite R scatter coordinates versus coarsened native full-count pixels (or exact points). This is a diagnostic aggregation, not an original R histogram or independent point-by-point rendering.
Q cutoff: 8; input SHA-256 prefixes: c71245ad265ed1a1. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![length_vs_q comparison](1d2-source_001-length_vs_q.png)

## rna · q_histogram · source_001

Counts on DIFFERENT grids: R 300 automatic Q bins; ONTRunQC fixed Q intervals. Zero-length Q population differs.
Q cutoff: 7; input SHA-256 prefixes: 1d1860dabc15ed69. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![q_histogram comparison](rna-source_001-q_histogram.png)

## combined · combined_length_histogram · combined

Counts on DIFFERENT grids: R 300 automatic log10 bins; ONTRunQC fixed bp intervals (centres shown). No binwise residual is defined.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![combined_length_histogram comparison](combined-combined-combined_length_histogram.png)

## combined · combined_q_histogram · combined

Counts on DIFFERENT grids: R 300 automatic Q bins; ONTRunQC fixed Q intervals. Zero-length Q population differs.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![combined_q_histogram comparison](combined-combined-combined_q_histogram.png)

## combined · combined_yield_by_length · combined

R cropped per-read cumulative path; ONTRunQC exact inclusive minimum-length queries. Ties, crop and bounded display are distinct.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![combined_yield_by_length comparison](combined-combined-combined_yield_by_length.png)

## combined · length_distributions · combined

Actual ggplot StatDensity vs opt-in R-grid Rust estimator; log-length KDE is computed AFTER log10 transform.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![length_distributions comparison](combined-combined-length_distributions.png)

## combined · q_distributions · combined

Actual ggplot StatDensity vs opt-in R-grid Rust estimator; log-length KDE is computed AFTER log10 transform.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![q_distributions comparison](combined-combined-q_distributions.png)

## combined · yield_over_time · combined

R exact cumulative-at-read-start queried every 15 min; ONTRunQC bin-end values. Within-bin paths differ; terminal bp conserved.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![yield_over_time comparison](combined-combined-yield_over_time.png)

## combined · yield_by_length · combined

R cropped per-read cumulative path; ONTRunQC exact inclusive minimum-length queries. Ties, crop and bounded display are distinct.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![yield_by_length comparison](combined-combined-yield_by_length.png)

## combined · length_by_hour · combined

Actual original-R fitted layer vs native Rust curve; original strict GAM failures remain failures in the separate differential suite.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![length_by_hour comparison](combined-combined-length_by_hour.png)

## combined · q_by_hour · combined

Actual original-R fitted layer vs native Rust curve; original strict GAM failures remain failures in the separate differential suite.
Q cutoff: 7; input SHA-256 prefixes: 6b9a356c4aa99b65, 1c77d6b888e49641. Axes/units/populations: A/B labels; C first group only unless caption states otherwise.

![q_by_hour comparison](combined-combined-q_by_hour.png)

Public fixture figures derived from Rob Lanfear's MinIONQC (MIT), pinned source and input hashes in [measurements.json](measurements.json). See [upstream license](https://github.com/roblanf/minion_qc/blob/54485d586038d8ee36f168dd94f31c423f52f581/LICENSE). Original scientific outputs are unchanged; rendering DPI alone is reduced.
