---
format: aep.planning-md/3
id: review-result:adversary-w8-20261008-loom-engineering-protocols-030-pin-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w8 adversary, loom story:engineering-protocols-030-pin, pass 1
relations:
- reviews: story:engineering-protocols-030-pin
revision: 1
---
# Wave 2026-10-08-w8 adversary, loom story:engineering-protocols-030-pin, pass 1

Tree `loom-20261008-w8-ep030` at `930687f` (base `5e21188`); cases committed as `5b843d1`. Builds
with `serde_json` `arbitrary_precision` on (`cargo tree --locked -e features -i serde_json -p
b10x-loom-governor`: turned on by `b10x-assertion-providers`, `b10x-canon-engineering-assertions`
0.3.0 and `b10x-canon-expr` 0.1.0) and off (`-p b10x-loom-executor` alone). Executed 73 to 82, 1 red.

- Red, introduced: `crates/loom-executor/src/session.rs:527`, `SessionFile::parse` reads a filed
  session into a `Value` and then `from_value`; with the feature on, an integer above `u64::MAX` in a
  tool-call item reaches `visit_u128`, which serde's internally tagged buffer does not implement, and
  the whole session is refused. The same file loads with the feature off. Case:
  `crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs`
  (`a_filed_session_whose_tool_call_holds_a_large_integer_loads`). Fix named: deserialize the typed
  file from the text after the version check.
- Confirmed, pre-existing: `crates/loom-protocols/src/lib.rs:117`, the engineering catalog's
  revision literal was tied to the manifest tag by no test; `crates/loom-protocols/tests/adversary_w8_pin.rs`
  now holds it.

Attacked without a break: numbers (7, -42, `u64::MAX`, `u64::MAX+1`, 0.1, 1e3) through the flattened
`ToolRequested` (`event.rs:287`) and the internally tagged `LoopEvent`, `LoopStop` and `Item` read from
text with the feature on; the executor's 407 unit tests with the feature on; one copy each of
`b10x-canon` (tag 0.1.0) and `b10x-canon-engineering` (tag 0.3.0); both fixtures byte-equal to the
release; no old tag or `branch = "main"` in manifests, `AGENTS.md`, `website/` or `README.md`; a
governor or protocol mutant that discharges or drops `investigate_cause` at `service-restored` is
caught by the per-state fixture check and the explicit assertions.

```findings
[{"file": "crates/loom-executor/src/session.rs", "line": 527, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "with arbitrary_precision unified in by engineering-protocols 0.3.0 and Canon 0.1.0, SessionFile::load refuses a filed session whose tool-call items hold an integer above u64::MAX, because from_value hands it to visit_u128, which serde's internally tagged buffer does not implement"}, {"file": "crates/loom-protocols/src/lib.rs", "line": 117, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "the engineering catalog's revision literal was tied to the manifest tag by no test, so a pin move that left it stale stayed green; adversary_w8_pin.rs now holds it"}]
```
