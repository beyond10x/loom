# Skipped conformance scenarios

Each line below names one conformance scenario this repository skips, and why:
`- <scenario id>: <reason>`. A line is added only for a scenario Loom cannot answer. The list is
empty when nothing is skipped.

A scenario named here does not keep `task conform` green: the Rust target reports a scenario it
cannot answer `unsupported`, ESS sets the report's `conformance_status` to `failed` for any
unsupported scenario, and `task conform` fails unless `conformance_status` is `passed`.
