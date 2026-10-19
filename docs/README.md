# ONTRunQC documentation

Use the [project README](../README.md) to install ONTRunQC and generate a first
report. This index separates operating instructions, scientific definitions,
compatibility evidence and historical measurements.

## Choose a guide

| Task | Authoritative guide | What it covers |
| --- | --- | --- |
| Install and run | [Project README](../README.md) | Installation, quick start and common commands. |
| Prepare input | [Input and run processing](input.md) | Plain/gzip TSV, modern/legacy schemas, RNA controls, discovery and run chunks. |
| Configure processing | [Input and run processing](input.md#bounded-parallel-processing) | Workers, CLI compatibility and cross-pass source integrity. |
| Interpret results | [Metric definitions](metrics.md) | Read populations, exact statistics, histograms, time/channel/mux diagnostics and optional estimators. |
| Consume or export reports | [Reports and exports](output.md) | Schema version 1, offline HTML, YAML, figure formats/dimensions and publication guarantees. |
| Assess MinIONQC compatibility | [Scientific comparison](minionqc-comparison.md) | Formula/plot mappings, actual evidence and current numerical differences. |
| Inspect figures | [Comparison gallery](figures/minionqc-comparison/README.md) | Original R, actual native output and numerical diagnostic panels. |
| Reproduce validation | [Reference harness](../tests/reference/README.md) | Pinned environment, public/R suites, classification and tolerance contracts. |
| Measure resource use | [Benchmarking](benchmark.md) | Procedures, dated measurements and [comparison figures](benchmark.md#benchmark-figures). |
| Review engineering history | [Parity ledger](minionqc-parity.md) | Acceptance matrices and preserved evidence by issue. |
| Contribute | [Repository guidelines](../AGENTS.md) | Architecture, scientific invariants, checks and commits. |

## Suggested reading paths

- **Sequencing facility:** quick start → input → reports → metric definitions.
- **MinIONQC comparison:** scientific comparison → gallery → discrepancy register
  linked there → optional reference suite.
- **Development:** repository guidelines → relevant metric/input/output reference
  → tests → historical ledger when investigating an existing decision.

## Evidence and scope

Functional coverage, matching scientific definitions, numerical plot layers,
visual rendering and CLI defaults are separate compatibility claims. The current
strict R suite still records **1,542 GAM fit/uncertainty discrepancies**; it is not
a passing full-parity suite. Native histogram/time/yield representations also
have documented differences. See the comparison before interpreting a historical
`Complete` label.

Benchmark observations describe the named inputs, builds and environments;
they are not general HPC throughput/RSS guarantees. Documentation restructuring
does not rerun or alter the committed scientific reference evidence.
