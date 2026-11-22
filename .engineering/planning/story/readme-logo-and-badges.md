---
format: aep.planning-md/3
id: story:readme-logo-and-badges
kind: story
status: proposed
title: Loom has a short README, a logo and local badges for code coverage and ESS coverage
summary: README of at most 110 lines with a quick start a test runs; logo.svg under 4 KB and a derived preview PNG and favicon; local SVG badges from committed JSON for coverage, ESS, tests, version, licence and release; task coverage in the slow lane; no coverage gate
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:test-step-within-budget
- depends_on: story:release-binaries
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: crates/loom-cli/tests
- confidence: cited
  path: crates/loom-conformance/tests/conform.rs
- confidence: inferred
  path: crates/loom-xtask/src/main.rs
- confidence: inferred
  path: crates/loom-xtask/tests
- confidence: inferred
  path: docs/assets/logo.svg
- confidence: inferred
  path: docs/badges
- confidence: cited
  path: website/docusaurus.config.ts
- confidence: inferred
  path: website/static/img/mark.svg
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T03:12:59Z", actor: "agent:loom", revision: 6}
---
## Outcome

Loom's front page is a README a stranger reads in a minute and remembers. It has a logo the
repository owns and a row of badges that the repository makes itself. Nothing on the page asks a
third party for anything.

**The README is for humans** (folder rule: `AGENTS.md` is for agents, `README.md` for humans,
`CLAUDE.md` only points to `AGENTS.md`). Today it is 763 lines (`README.md:1-763`), most of them
release history and command reference. It is rewritten into these sections, each held to its
length by a test:

1. **Header**, at most 8 lines: the logo (`docs/assets/logo.svg`, 96 px), the title, one line
   saying what Loom is, and the badge row.
2. **What Loom is**, at most 10 lines: a governed agent runtime; a model acts for one bounded
   run, and only through an action the current frontier admits; Canon decides what is
   legitimate; Commission rechecks every proposal before an effect runs; the host owns
   credentials, merges and deploys. The last line links to "Where Loom ends".
3. **How a step runs**, at most 12 lines: a diagram in a `text` block, cheaper than a paragraph.
   It shows intent, protocol, case, frontier, selector, proposed action, Commission's recheck,
   effect, evidence, and back to the governor.
4. **Quick start**, at most 30 lines (see below).
5. **Where next**, at most 10 lines: links to the website, Getting started, the guides (run an
   intent, bindings, embed the runtime, deploy to Kubernetes, run the checks), the status page and
   `CHANGELOG.md`.
6. **Crates**, at most 4 lines: the one crate an embedding depends on (`b10x-loom-sdk`) and the
   CLI, then a link to the generated crate reference (`website/docs/reference/crates.md`). The
   table of `README.md:720-751` moves there.
7. **Contributing**, at most 6 lines: `task check`, its tools, the link to "Run the checks", and
   that agents read `AGENTS.md`.
8. **Licence**, at most 2 lines.

The whole README is at most 110 lines. It uses no marketing words: the test refuses a list of
them (`powerful`, `seamless`, `blazing`, `revolutionary`, `cutting-edge`, `effortless`,
`next-generation`, `world-class`).

What leaves the README moves; nothing is dropped:

- the "Since x.y.z" paragraphs (`README.md:9-162`) are already in `CHANGELOG.md`;
- "Run the command line" (`:163-363`) goes into `website/docs/guides/run-an-intent.md`;
- "Deploy agents to Kubernetes" (`:364-603`) becomes the new page
  `website/docs/guides/deploy-to-kubernetes.md`;
- "Embed the runtime" (`:604-719`) goes into `website/docs/guides/embed-the-runtime.md`.

**The quick start** runs from a clean checkout. It needs no model, no network after the install,
and no delegated cgroup:

- install: download the release binary and check it with `sha256sum -c`, as
  `story:release-binaries` gives it, or build from source with
  `git clone --branch <version> …`, `cd loom` and `cargo install --locked --path crates/loom-cli`;
- `b10x-loom --version`;
- `b10x-loom protocols list`;
- `b10x-loom evaluate --input examples/quick-start/evaluate.json`, which prints Canon's decision
  for a small committed case: the frontier's admissible actions and the open obligations.

Each command is followed by its output, pasted from a run. Who checks which command:

- the three commands after the install are run by a test of `crates/loom-cli`, with the built
  binary, comparing the printed output;
- the install lines are not run (they need the network and minutes of build). A second test checks
  that they name the workspace version and paths that exist, and `story:release-binaries`' test
  `readme_and_site_download_the_release_binary_with_its_checksum` checks the download and
  checksum lines;
- `task docs-check` runs none of them.

The README names the binary and its checksum command, so this story depends on
`story:release-binaries`.

**The logo.** One SVG the repository owns, `docs/assets/logo.svg`, written by an agent as code.
No external service is used, no model is called in a test, and nothing about the project leaves
the machine. The concept: a loom's plain weave of four vertical warp threads (the cases of a tree)
and three horizontal weft threads (the evidence) crossing over and under each other.

- **Size:** under 4 KB, a `viewBox` of `0 0 64 64`, and a transparent background.
- **Colours:** warp `#1f8a70` and weft `#c0682a`. Both work on white and on dark backgrounds, so
  there is no dark variant. Against white `#ffffff`, the ratios are 4.26:1 and 4.00:1. Against
  GitHub's dark `#0d1117`, they are 4.44:1 and 4.74:1. Against the site's dark `#101a1b`, they are
  4.16:1 and 4.43:1. Each is above the 3:1 WCAG 2.2 sets for graphics (1.4.11). The two threads
  differ in direction and in over/under, not in colour alone.
- **Derived files:** `task assets` (`loom-xtask assets`) writes
  `docs/assets/social-preview.png`, 1280 × 1280, rendered from the SVG in process by the Rust
  `resvg` crate, and `website/static/img/logo.svg`, a byte copy. `docusaurus.config.ts:11`
  points its `favicon` at that copy in place of `img/mark.svg`.

**The badges** are SVG files under `docs/badges/`, each rendered by `task badges`
(`loom-xtask badges`) from a committed JSON file beside it. No badge comes from shields.io or any
other service. The README shows them as relative paths, so github.com serves them from the
repository itself: viewing the README makes no request to a third party that carries the
repository's name. These are the badges, what each shows, its source and its JSON:

- **coverage**: workspace line coverage in %, from `task coverage` (below); `coverage.json`
  holds the figure per crate and for the workspace.
- **ESS**: `<executed>/<declared>` scenarios, from `task ess-coverage` (below); `ess.json`
  holds each suite.
- **tests**: the count of tests in the JUnit report of the last `task test-step`
  (`target/nextest/default/junit.xml`, `crates/loom-xtask/src/test_step.rs:112`);
  `tests.json`.
- **version**: `[workspace.package] version` (`Cargo.toml:7`); `version.json`.
- **licence**: `license` (`Cargo.toml:9`) and `LICENSE`; `licence.json`.
- **release**: the last release and its date, from the newest `## [x.y.z] - date` heading of
  `CHANGELOG.md` (`:69`); `release.json`.

The release badge reads `CHANGELOG.md` and not `git describe`, because a CI checkout may hold no
tags. `release-check --tag` already holds the heading to the tag. Under Loom's release rule
(`AGENTS.md` § Releases, step 1) the version and the release are equal after every release commit.
The version badge says what `main` builds, and the release badge links to the release.

**The ESS badge is one combined badge**, the sum over the three conformance suites that
`task check` runs: `task conform` (`Taskfile.yml:158-161`), `task intake-conform` (`:163-166`)
and `task commission:conform`. Executed is `counts.passed + counts.failed` of each report, as
`crates/loom-conformance/tests/conform.rs:279` counts it, and declared is `counts.total`. One
figure is what a stranger reads. `ess.json` keeps each suite's `passed`, `failed`, `error`,
`skipped`, `unsupported`, `total` and `coverage.knowledge`. It also keeps the gate's figure from
`task ess-gate` (`:66-69`): the refusal occurrences of `generated/ess/loom/synthesis.txt` and how
many authored scenarios cover, read as `crates/loom-executor/tests/ess_gate.rs:211-260` reads
them. The report files are removed after a passing run (`conform.rs:163-174`), so each conform
test gains an optional output, `LOOM_CONFORM_COUNTS=<file>`, to which it writes its report's
counts. `task ess-coverage` sets it.

**Colours, fixed now.**

- Coverage and ESS: green `#1a7f37` at 80 % or more, yellow `#9a6700` from 60 %, red `#b42318`
  below 60 % or with any failed scenario.
- The other badges: blue `#0969da`.
- The label: `#555555`.
- With white text, the ratios are 5.08, 4.87, 6.57, 5.19 and 7.46:1.

No coverage figure fails a build. The first measurement is not guessed here; a follow-up story
may set a ratchet from it.

**Freshness.** Each JSON records the `commit` and the date it was measured at, and each SVG
carries both in its `<title>`. `task badges-check` (`loom-xtask badges --check`, added to
`task check` after `docs-check`, `Taskfile.yml:20-44`) takes well under a second. It fails,
naming the badge, in these cases:

- An SVG differs from its JSON as rendered.
- A derived badge (version, licence, release) records a SHA-256 of its source (the version
  string, the `license` value with `LICENSE`, the `CHANGELOG.md` heading) that differs from the
  source's digest now, the same rule as the committed suite's input digest.
- A measured badge (coverage, ESS, tests) records another `version` than the workspace version.
  Measured badges are regenerated once per batch gate or per release, not per unit. A digest of
  the code they measure would change in every unit, and parallel units would then conflict on one
  file. So the release commit, which moves the version, must regenerate them. `AGENTS.md`
  § Releases step 1 gains `task coverage`, `task ess-coverage` and `task badges`.

**`task coverage` is in the slow lane**, never in the test step, as `~/beyond10x/AGENTS.md`
§ Tests stay fast requires. It runs `loom-xtask coverage`:

1. `cargo llvm-cov clean --workspace`;
2. per workspace crate, one `cargo llvm-cov nextest --no-report -p <crate> --locked` under the
   default nextest profile, so it keeps the 60 s kill and leaves out `slow_` tests. Each crate is
   a separate command through `slot`, meant for a quiet pool;
3. one `cargo llvm-cov report --json --summary-only`, which merges every crate's profile.

It measures the lines of `crates/*/src`. It excludes:

- `crates/*/tests/`, which is not product code;
- `generated/`, which ESS synthesizes: it would make the figure about the generator, and the ESS
  badge covers it;
- `consumers/`, which is outside the workspace.

Lines of inline `#[cfg(test)]` modules in `src` files are counted, because stable `llvm-cov` cannot
leave them out, and `coverage.json` says so. A test killed at 60 s under instrumentation is
listed in `coverage.json` as `killed`, and that crate's figure is marked incomplete. The workspace
figure is total covered lines over total lines, not a mean of the crates' percentages.

**Order.** This story comes after `story:test-step-within-budget` (the step is under 120 s first)
and `story:release-binaries` (the README names the binary and its checksum). No story depends on
it, so it is off the Runbook stub-cycle path.

## Why

Timo, 2026-11-22: "make sure Loom gets also nice badges in the README, that it gets a nice logo
and a polished README, one that you remember when you go there and come back. The badges should
hold code coverage, ESS coverage." Today the README opens with 150 lines of release history before
its first command (`README.md:9-162`). The site has a 300-byte placeholder mark, an `L` in a
rounded square (`website/static/img/mark.svg`). No figure shows how much of Loom its tests or its
specification reach.

## ESS first

`ess/` does not change: the badge and asset files are repository tooling in `loom-xtask`, and no
product type, command or refusal is added. `slot task drift` stays green.

The first commit adds the tests below. They fail because `docs/assets/logo.svg`,
`docs/badges/` and the README's `## Quick start` do not exist, and the `assets` and `badges`
commands of `loom-xtask` do not compile. That is the red check the unit records.

## Acceptance

The tests use no model and no network, and each ends within 5 s. The quick-start test runs the
built `b10x-loom` three times with an isolated `XDG_DATA_HOME`, as
`crates/loom-cli/tests/evaluate.rs:23-40` does. The test step stays within 120 s, and no test runs
over 60 s. `task coverage` and `task ess-coverage` are not part of the test step. The new tests
are modules of `tests/main.rs` under `story:one-test-binary-per-crate`; no test target is added.

- The logo is well-formed XML with an `<svg>` root, a `viewBox` and fewer than 4096 bytes. It has
  no `<script>`, no `<foreignObject>`, no `on*` attribute, and no `href`, `xlink:href` or `url()`
  but a `#fragment`. Test: `the_logo_is_a_small_well_formed_svg_with_no_external_reference`.
- Every `fill` and `stroke` colour of the logo holds at least 3:1 against `#ffffff`, `#0d1117` and
  `#101a1b`. Test: `the_logo_colours_hold_their_contrast_on_light_and_dark`.
- `docs/assets/social-preview.png` is 1280 × 1280 and equals a fresh render of the logo, and
  `website/static/img/logo.svg` equals the logo byte for byte. Test:
  `the_social_preview_and_the_favicon_are_derived_from_the_logo`.
- Every SVG under `docs/badges/` equals its JSON as rendered, is well formed, names no external
  reference, and carries the JSON's `commit` and date. Test:
  `every_badge_is_its_json_rendered_and_names_its_measurement`.
- On a copy of the tree whose `Cargo.toml` names another version, `badges --check` exits non-zero
  naming the version badge and every measured badge. A copy with a changed `LICENSE` fails naming
  the licence badge. Test: `a_badge_older_than_its_source_fails_the_fast_check`.
- 80.0 % renders green, 79.9 % yellow, 59.9 % red, and an ESS figure with one failed scenario
  red. Test: `badge_colours_follow_the_fixed_thresholds`.
- `coverage.json`'s crates sum to its workspace figure, and its exclusions list `generated/`,
  `crates/*/tests/` and `consumers/`. Test: `the_coverage_json_sums_its_crates`.
- `ess.json`'s combined figure is the sum of its suites, and the ESS badge shows it. Test:
  `the_ess_badge_sums_its_suites`.
- Every URL in `README.md` has the host `github.com` (under `beyond10x/`) or
  `beyond10x.github.io`. Every image is a relative path under `docs/`, and no image has a URL.
  Test: `the_readme_names_no_third_party_host`.
- The commands of the README's `## Quick start` after the install print the output pasted below
  each. Test (in `crates/loom-cli`): `the_readme_quick_start_runs`.
- The quick start's install lines name the workspace version and paths that exist in the tree.
  Test: `the_readme_install_lines_name_the_current_version`.
- The README has the eight sections in order, each within its length, at most 110 lines in all,
  and none of the listed marketing words. Test: `the_readme_keeps_its_sections_and_lengths`.
- Each link of `## Where next` resolves to a page under `website/docs/`, and
  `deploy-to-kubernetes.md` exists. Test: `the_readme_links_the_site_and_its_guides`.
- `the_upgrade_guide_applies_the_crd_of_the_workspace_version`
  (`crates/loom-operator/tests/chart.rs:459-508`) reads the Kubernetes guide page in place of
  `README.md` and passes. `readme_and_site_download_the_release_binary_with_its_checksum` and
  `crates/loom-docs/tests/adv_q13_agent_stack_layers.rs` pass unchanged.
- `task check` calls `badges-check` and calls neither `coverage` nor `ess-coverage`. Test (reads
  `Taskfile.yml`): `coverage_is_a_slow_lane_task_and_not_part_of_the_check`.
- `slot task docs-check`, `slot task website` and `slot task check` exit 0.

## Out of scope

- A redesign of the documentation site.
- Publishing to a registry.
- Signing a release or a badge.
- Uploading the social preview in the repository's settings, a GitHub setting that waits for Timo
  or `host-operator`.
- A coverage gate or ratchet, and coverage in CI.
- Badges from any service.
- A quick start that runs a case tree: it needs `story:tree-run-command` and a delegated cgroup,
  and it is a follow-up once that story lands.
- Changes to Substrate, to Canon, and to the CLI's default.

## Scope

Derived 2026-11-22 from the plan tree at `1ee45331`. **Cited** means read at the `file:line`
named. **Inferred** means a reading that could be wrong.

- **Primary surface:** `README.md` (`:1-763`, sections at `:1`, `:28`, `:46`, `:163`, `:364`,
  `:604`, `:720`, `:752`, `:761`) — cited
- **Files (new):** `docs/assets/logo.svg`, `docs/assets/social-preview.png`,
  `docs/badges/{coverage,ess,tests,version,licence,release}.{json,svg}`,
  `examples/quick-start/evaluate.json` (`examples/` holds `agents/` today),
  `website/static/img/logo.svg`, `website/docs/guides/deploy-to-kubernetes.md` — cited for the
  directories, inferred for the names
- **Files (tooling):** `crates/loom-xtask/src/main.rs` (the `Command` enum `:88`; new `assets`,
  `badges`, `coverage` and `ess-coverage` commands); `crates/loom-xtask/Cargo.toml` (`resvg`, an
  XML reader) — cited for the enum, inferred for the dependencies
- **Files (tasks):** `Taskfile.yml`: `check` (`:20-44`) gains `badges-check`; new `assets`,
  `badges`, `coverage` and `ess-coverage` tasks — cited
- **Files (conform tests):** `crates/loom-conformance/tests/conform.rs` (the counts `:255-279`,
  the scratch report `:163-174`), `crates/loom-intake-conformance/tests/conform.rs`, the
  commission conformance test of `Taskfile.commission.yml` — cited for the first two, inferred
  for the third
- **Files (tests):** `crates/loom-xtask/tests/` (the logo and badge tests), `crates/loom-cli/tests/`
  (the README tests, beside `evaluate.rs`), `crates/loom-operator/tests/chart.rs:459-508` (reads
  the new guide) — cited
- **Files (site):** `website/docusaurus.config.ts:11` (`favicon`); `website/static/img/mark.svg`
  is removed once nothing else reads it; `website/docs/guides/run-an-intent.md` and
  `embed-the-runtime.md` take the moved sections; `website/sidebars.ts` lists the new guide —
  cited for the files, inferred for `mark.svg`'s other readers
  (`website/product.json` names a text mark `L`, not the file)
- **Files (docs):** `AGENTS.md` § Generated files (`:416-433`) gains the asset and badge rows;
  § Releases step 1 (`:2708-2717`) gains the three tasks; § Documentation (`:2746-2755`) names the
  README rule; `CHANGELOG.md` one `Changed` bullet under `[Unreleased]` — cited
- **Not changed:** `ess/`, `generated/`, `.github/workflows/check.yml` (it runs `task check`,
  `:103-104`, so `badges-check` runs on CI with no workflow change) — cited
- **Would collide with:** `story:release-binaries` on `README.md`'s install lines (`:170-185`) and
  `website/docs/getting-started.md` (it lands first) — cited from its Scope;
  `story:test-step-within-budget` on `Taskfile.yml` and `crates/loom-xtask` (it lands first) —
  cited; `story:one-test-binary-per-crate` on the test layout — inferred
