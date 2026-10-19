# Reports, schema and figure export

Reference for report files, JSON schema, offline HTML, YAML and independent scientific figures.
See [metric definitions](metrics.md) for populations/formulas and the [documentation index](README.md) for validation evidence.

## Contents

- [Single-run and collection output](#single-run-and-collection-output)
- [Report envelope and ordering](#report-envelope-and-ordering)
- [HTML presentation](#html-presentation)
- [YAML and plot controls](#yaml-and-plot-controls)
- [Native static figure exports (#15)](#native-static-figure-exports-15)
- [Publication and existing files](#publication-and-existing-files)

## Single-run and collection output

```text
qc_results/                         collection_qc/
├── report.json                     ├── runs/
├── report.html                     │   ├── run_001/report.{json,html}
├── summary.yaml        [optional]  │   └── run_002/report.{json,html}
└── figures/            [optional]  └── combined/report.{json,html}
    └── manifest.json
```

JSON and HTML are always written. `--yaml` adds a summary; `--format` selects one
independent figure format per invocation. `--combined-only` omits individual
outputs. Generated ordinal names avoid sample identifiers in output paths.
Timelines and channel IDs remain local to each run; pooled medians/N50 use exact
pooled observations, never averages of run statistics.

```sh
ontrunqc analyze sequencing_summary.txt --output qc_results/ --yaml \
  --format pdf --smallfigures --add-stat --muxes 8
```

## Report envelope and ordering

JSON schema version remains **1**. Changes are additive; consumers should ignore
unknown fields, while strict unknown-key consumers must update. Counts/yields are
integer bp/read counts, floating outputs are finite, and unavailable statistics
are `null`. `tool.name` is `ONTRunQC`; `tool.version` is the Cargo package version.

| Top-level fields | Purpose |
| --- | --- |
| `schema_version`, `tool`, `run` | Schema/program version and available run identity. |
| `metrics`, `quality`, `read_length_distribution` | Core all-basecalled counts, yield, exact statistics and fixed distributions. |
| `channels`, `muxes`, `time_bins`, `end_reasons` | Ordered local groups and read-start intervals. |
| `diagnostics`, `comparison`, `distributions` | Deep diagnostics, independent Q-cutoff populations and default density estimates. |
| `input`, `presentation` | Validated input capabilities/exclusions and requested report settings. |
| `source`, `collection` | Optional source identity and exact pooled/per-run comparisons. |
| `fitted_trends`, `minionqc_density` | Optional raw-read fits and R-grid density estimates. |
| `minionqc_plot_bins` | Opt-in reference histogram/Bin2D layers with explicit summary-read populations; omitted by default. |

Channel/mux arrays sort numerically, time bins by start, histogram bins by lower
bound, and `end_reasons` by Rust string order. The separate diagnostic termination
array sorts by descending record count, then name. Fields follow the declared
schema order; pretty JSON ends with a newline. No run timestamp, original path,
read identifier or original TSV record is embedded. Explicit MinION reports may
include small-input numeric display points or bounded raster counts/extrema.

Header-only input has null run identity/statistics, zero counts/yields, empty
groups and zero-count fixed histograms. Zero-length-only runs retain available
identity/grouped record counts while basecalled length/Q statistics remain null.

## HTML presentation

`report.html` renders the final report model, including explicitly selected
MinION display payloads, without changing JSON schema 1. Pass rate is `100 * passed_reads /
basecalled_reads`, defined centrally in the metrics implementation; it is
unavailable when there are no basecalled reads. Other HTML conversions only
format units, round display labels, sort groups, or scale SVG coordinates.

The single-run cumulative overview steps at occupied bin ends and holds
constant across omitted intervals. All/Q-cutoff and per-run comparisons join
adjacent bin-end cumulative points; they do not reconstruct the original per-read
path. Interval
charts preserve actual start/end coordinates so gaps remain visible. The ranked
channel plot displays one existing channel yield per point, with descending
yield and ascending channel ID for ties. It carries no physical coordinate
information. Histograms retain the original categorical interval boundaries;
no per-read distribution is inferred. All table counts and bp yields are exact
integers; unknown floating statistics display **Not available**.

| Report section | Scientific content |
| --- | --- |
| Overview | Summary/basecalled counts, yield, native pass rate, N50, mean length and Q-score. |
| Yield and temporal QC | Cumulative/interval yield, read counts, observed channel activity and all/passed means. |
| Length and quality | Fixed histograms, exact all/passed statistics and joint length × Q heatmaps. |
| Channels and mux | Four channel distributions, exact medians, detailed tables and optional physical maps. |
| Termination and speed | Reason counts/fractions/yield and eligible template length/duration observations. |
| Q-cutoff comparison | Separate selected population, exact length thresholds, retained-yield curve and ultralong subset. |
| Combined reports | Pooled summaries and per-run distribution, yield and temporal comparisons. |
| Optional reference views | Raw-read fitted trends and R-grid density estimates when explicitly enabled. |

## YAML and plot controls

`presentation` is additive schema-version-1 configuration: `write_yaml`,
`export_svg`, `small_figures`, `add_stat`, optional `mux_interval_hours`, and
centrally prepared `mux_markers_seconds`. It never changes QC populations or
histogram definitions. Existing consumers must tolerate additive fields.

`--yaml` emits `summary.yaml`, using quoted YAML mappings and JSON-compatible
scalars (YAML 1.2), without a new runtime dependency. Sections `All reads` and
`Q>=<cutoff>` project canonical exact statistics. They contain `total.reads`,
`total.bases` (exact integer extension), `total.gigabases`, `N50.length`,
`mean.length`, `median.length`, `max.length`, `mean.q`, `median.q`, `reads`, and
`gigabases`. Threshold labels match upstream (`>10kb` through `>1m`) but selection
is **inclusive**, as in its code. `ultralong` is the existing maximal prefix.
Unavailable statistics are null; values are unrounded, unlike upstream's
one-decimal summaries. Input identification uses generated source labels and
run IDs, not original paths. Raw summary/control counts and notes make the
positive-length All-read denominator explicit. Native passing remains separate.

Configured mux references use relative hours, starting at zero, through the
final occupied interval's end. Zero disables references (default); positive
intervals producing more than 512 markers fail actionably. References share the
same axes in individual/comparison charts. They are not telemetry, occupancy or
inferred mux switching. Cumulative comparison points sit at bin ends; mean/count
observations sit at midpoints. Missing intervals break mean lines.

`--add-stat` places exact-model mean length/N50 and mean Q annotations in the
corresponding histogram SVGs. Display rounding/unit conversion is presentation
only. `--format svg` exports every nonempty scientific SVG, with embedded styles,
internal accessible titles and a current-file manifest. Width is 180 mm, or
120 mm with `--smallfigures`;
wide physical figures retain 320 mm. Height preserves aspect ratio. Empty figures
stay explanatory states in HTML, rather than meaningless SVG files. Old SVGs not
listed in a new manifest are retained, not silently removed.

Native conversions below produce independent artifacts, not browser printouts.
See [publication guarantees](#publication-and-existing-files) for staging and
replacement behavior.
See [the parity matrix](minionqc-parity.md) for the full audit and tests.

## Native static figure exports (#15)

`--format png|pdf|ps|jpeg|tiff|bmp` writes every nonempty scientific figure, using
the same self-contained SVG extraction as `--format svg`. JSON and HTML remain
mandatory outputs. `presentation.export_format` is an additive, lowercase string
only for these six formats; it is omitted for HTML/SVG. Existing `export_svg` and
all scientific fields keep their meanings; schema version stays **1**. Strict
unknown-field consumers need to accommodate this rendering metadata.

Physical widths stay 180 mm /120 mm small mode /320 mm wide maps; heights preserve
the canonical figure aspect ratio, captions, legends and statistical annotations.
SVG CSS palette variables become equivalent literals, since SVG 1.1 processors
do not implement browser custom properties. Data/axes/bin boundaries never change.

- Rasterization: `usvg`/`resvg`, 96-DPI SVG unit parsing, then **300-DPI** output,
  pixel dimensions rounded to nearest integer. White compositing and in-place
  RGBA→RGB compaction avoid retaining a second full bitmap. PNG/TIFF/BMP are
  lossless 8-bit RGB; JPEG is quality 95 and lossy. PNG/BMP store 11,811 pixels/m;
  JPEG/TIFF store 300 pixels/inch. Colors follow the SVG's sRGB values; these
  are not CMYK/prepress-color-managed outputs.
- PDF: `svg2pdf` preserves paths and selectable, subsetted Liberation Sans text.
  SVG 96-DPI units are converted to 72-point/inch pages. Existing embedded PNGs
  remain raster images, losslessly encoded; vector export does not reconstruct
  individual reads from pixel aggregation. PDF is not claimed to be PDF/A.
- PS: a focused native PostScript **Level 3** image-page writer, using lossless
  Flate/ASCIIHex RGB data. Physical `PageSize` and integer/HiRes bounding boxes
  match the source SVG. The chosen SVG libraries have no PS writer; this
  documented raster-backed approach avoids a second vector/font engine or
  an external converter. All content is 300 DPI, including text; scaling is
  resolution-limited. No disguised PDF/SVG payload or runtime Ghostscript.
- Every converter processes only the current SVG. MinION's shared atlas is
  decoded once, then losslessly cropped into equivalent channel tiles before
  native rendering, preserving original geometry, alpha and all extrema.
  This avoids repeatedly decoding/transforming the complete atlas for 512 uses.
  Interpolation at tile boundaries can differ from a shared clipped image;
  original atlas/count model and shared-atlas SVG representation remain unchanged.
- Rendering rejects allocations above 64 million pixels, with SVG/PDF or small
  mode suggested. This guard is not a whole-process RSS guarantee. Grids,
  existing exact-statistics arrays, renderer trees and encoder buffers still
  consume memory. Temporary bitmap/tree storage is released between figures;
  all completed outputs remain staged on disk until publication.

The default `static-figures` Cargo feature includes the Rust SVG/rendering and
codec dependencies; their licenses are MIT/Apache-2.0-compatible. `image` is pinned
to 0.25.9 to preserve Rust 1.85; resvg/usvg 0.45 and svg2pdf 0.13 share one parser.
No Rayon/default image-format bundle, browser or C graphics/font library is
required. Liberation Sans Regular/Bold 2.1.5 is bundled unmodified under SIL OFL
1.1, with its license in `assets/fonts/` and release archives. Fonts are loaded
from memory; no system-font discovery occurs. Minimal `--no-default-features`
builds retain JSON/HTML/SVG and reject conversion requests before analysis.

Figure names and manifest order are stable. Raster/PS payloads are reproducible
with the locked renderer; PDF resources may be serialized in different orders,
so byte-for-byte PDF identity is not guaranteed. This does not change scientific
JSON, page geometry or plotted observations. JPEG quantization and PDF/raster
font antialiasing differences are not numerical QC discrepancies.

Unit/CLI tests decode raster formats, verify dimensions/density, embedded-image
colors, white compositing, deterministic names and failure cleanup. The ignored
`figure_render::tests::external_pdf_postscript_decoders` test uses actual Poppler
and Ghostscript, and CI runs it explicitly. Optional `tests/static_exports.py`
compares all six formats against canonical SVGs on synthetic and pinned public
inputs, including fitted/combined charts and exact/dense MinION plots. It checks
lossless RGB fidelity using independent decoders. See [executed measurements](benchmark.md#native-static-figure-exports-15).

## Publication and existing files

All requested files are completed and synced in temporary destination files
before replacement. Predictable failures clean staged outputs and preserve prior
valid reports. Renames replace files individually; publication is **not atomic
across files or runs**. A later failure may leave complete old/new files mixed.
Check errors before using output from a failed command.

`figures/manifest.json` lists the current requested figures in deterministic order.
Older unlisted figures and optional outputs not requested again are retained.
Do not treat every file left in a reused directory as part of the new report.
Source revalidation and overwrite protection are defined in
[input integrity](input.md#multi-pass-input-integrity).
