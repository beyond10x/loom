# Skipped conformance scenarios

Each line below names one conformance scenario this repository skips, and why:
`- <scenario id>: <reason>`. A line is added only for a scenario Loom cannot answer. The list is
empty when nothing is skipped.

A scenario named here keeps `task conform` green while ESS rates the run failed: the Rust target
reports a scenario it cannot answer `unsupported`, and ESS sets the report's `execution_status` and
`conformance_status` to `failed` for any unsupported scenario.
