---
format: aep.planning-md/3
id: review-result:ess-format-37-acceptance-1
kind: review-result
status: active
title: Plan critic (acceptance), round 1, on story:ess-format-37
relations:
- reviews: story:ess-format-37
revision: 1
---
needs-revision

story:ess-format-37 — line 85-86 checks the suite only by its count and its scenario ids, so it does not check what the story rests on: the compiled model and the suite keeping their bytes (lines 29-32 and 75-76). The command also has no `--out`, and the help for `ess verify conform synthesize` says the `ir` suite is written "to the file `--out` names", so there is no named file to read the ids from — .engineering/planning/story/ess-format-37.md:85
- Evidence: the probe shows `sha256sum suite-31.json suite-37.json` both `c81a8ab69cae…408cedf` and `ir-31.json ir-37.json` both `ead26995ea1d…b40b0`. Only bytes show nothing changed. Ids can match while the assertions differ.
- Fix: replace the bullet with "`ess verify conform synthesize --path ess --out S` on the parent, then `ess verify conform synthesize --path ess --out S --check` on the commit exits 0 and reports `181 scenario(s) (0 authored), 0 refusal(s)`; `ess specify compile --path ess --format json` prints the same sha256 on both commits."

story:ess-format-37 — the one commit puts `CHANGELOG.md` next to `ess/system.yaml`, but line 75 claims an exemption only "from a red test". AGENTS.md § ESS says "A unit's first commit changes only `ess/`" and exempts a no-behaviour change only when "its story says so". The model story kept them apart: `4ab05d5b` changes only `ess/system.yaml` and the CHANGELOG bullet came in `8b69b0b6` — .engineering/planning/story/ess-format-37.md:24
- Evidence: AGENTS.md:231-235; `git log --name-only d265680b^2 -4`.
- Fix, either one:
  - Line 75: "Exempt (AGENTS.md § ESS) from a red test and from a first commit limited to `ess/`: …".
  - Or split into two commits. Line 24 becomes "two commits: `ess/system.yaml` (one line), then one `CHANGELOG.md` bullet". Bullet 2 (line 82) becomes "The first commit changes only `ess/system.yaml` (one line), the second only `CHANGELOG.md`; …".

Checked and correct:
- All 9 bullets can fail.
- Bullets 1 and 8 carry the before/after change: today `git grep -n "format: ess/31" -- ess/` prints `ess/system.yaml:1`.
- Bullets 4-6 are unchanged-before-and-after checks, which suits a story that changes no behaviour.
- The "exactly two files" part of bullet 2 covers both Outcome promises that have no bullet of their own: "no ESS toolchain moves" and the six `ess/31` places left as they are.

What I read: 1 artifact (`story:ess-format-37`, the whole body, from its file), plus `story:ess-format-31` (acceptance and ESS first), AGENTS.md § ESS, Gate, Generated files and Releases, and the probe's logs, suites and scripts.
- Commands run: `ess specify validate --path ess --strict-requires` in the tree (`loom v1 — 16 file(s), 97 scenario(s), valid`, exit 0), `ess --version` (0.96.0), `git grep`, `git log`, `sha256sum`.
- Every cited number matches the probe:
  - 181 scenarios, 0 authored, 0 refusals, 7 notes in both synth logs, `ess-conformance/34`.
  - Validate prints the 16/97 line at `ess/31` and at `ess/37`.
  - The 77 files are 23+27+23+2+1+1: the docs-generate log wrote 23, 24+3 and 2, and the crd-generate log names the deploy and chart files.
  - All 10 tasks exit 0 in `logs/status.txt`.
  - Both cargo test runs exit 0 in `logs/test-status.txt`.
  - The probe tree's `git status` names `ess/system.yaml` only.
- Every `file:line` matches: `ess/ess-inputs.yaml:2`, `check.yml:60-61`, `pages.yml:35-36`, and the six `ess/31` places.
- Small correction to the brief: the suites are in `ess37/` itself, not in the directory above.

Could not establish:
- What `ess verify conform synthesize --path ess` does with no `--out`. I did not run it because it might write into the tree.
- Whether a format raise is "a change a consumer sees" (AGENTS.md § Releases). The model story set the precedent of one bullet under `### Changed`.
- Out of my lane, for the design or scope critic: `story:decision-request-entity` .engineering/planning/story/decision-request-entity.md:231 and `epic:decision-requests-in-the-runtime` .engineering/planning/epic/decision-requests-in-the-runtime.md:417 still require `format: ess/31`, and the first also requires no `ordered:` key. Both fail once this story lands.

```findings
- file: .engineering/planning/story/ess-format-37.md
  line: 85
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the bullet checks the suite only by its count and its scenario ids and names no --out file, so it does not check what the story rests on (lines 29-32 and 75-76): the compiled model and the suite keeping their bytes"
- file: .engineering/planning/story/ess-format-37.md
  line: 24
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the one commit puts CHANGELOG.md next to ess/system.yaml, but the ESS first section claims an exemption only from a red test, not from AGENTS.md § ESS's first commit that changes only ess/"
```
