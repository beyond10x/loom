---
format: aep.planning-md/3
id: story:website-ux-loop
kind: story
status: proposed
title: Loom's docs website passes a measured layout, accessibility and screen-only user loop on every site change, with a ratchet baseline
summary: 'task website-ux lane (own 120 s budget, path-filtered, not in the test step): Playwright layout and axe gates in both themes, a screen-only bot over six tasks, a ratchet baseline and findings records; L4 to L8 and model calls are follow-ups'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:test-step-within-budget
- depends_on: story:planning-only-pull-requests-skip-build
- depends_on: story:readme-logo-and-badges
scope:
- confidence: cited
  path: .github/changed-paths.sh
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: crates/loom-xtask/src/ux
- confidence: inferred
  path: website/package-lock.json
- confidence: cited
  path: website/package.json
- confidence: inferred
  path: website/ux
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T03:19:18Z", actor: "agent:loom", revision: 6}
---
## Outcome

A `task website-ux` lane holds the docs website of Loom (`website/`) to the hard gates and
ratchets of the `ui-iteration-loop` skill, at the levels that need no model and no key: L1, L2,
L3, the ratchet against `website/ux/baseline.json` and the findings records. It runs apart from
the test step, on a pull request that changes what the site is built from, within 120 s, on the
shared pool and on the forge alike. A page that scrolls sideways, a control out of view, a
serious accessibility violation or a reader who can no longer reach an answer fails the merge,
and a measured value can only get better.

The rules, numbered as decided:

**1. Adapter.** One file, `website/ux/adapter.ts`, holds the slots of skill § 1. The generic code
of `website/ux/` (probes, bot, metrics) reads only the adapter and imports nothing from the
site's pages.

- **Scenarios:** every route of `website/build/sitemap.xml` (the landing page and the pages of
  `website/docs/`, 50 files, 27 of them generated), plus six reader tasks. Each task has its
  question, the words a reader scans links for, its target route and the answer text that must
  be in view:
  1. `install`: find the install command of `b10x-loom`. Target `docs/getting-started`, answer
     `cargo install --locked --path crates/loom-cli` (`getting-started.md:19`).
  2. `program-environment`: find how a bound program gets its environment. Target
     `docs/guides/bindings#running-a-program-binding`, answer `alone in its environment`
     (`bindings.md:226`, `:238-240`).
  3. `tree-approve`: find what `tree approve` does. Target `docs/reference/cli`, section
     `tree approve`. Requires `story:tree-run-command`, which adds the command (its body, `:77`).
  4. `release-checksum`: find the release binary and its checksum command. Target
     `docs/getting-started`, answer `sha256sum -c`. `story:release-binaries` adds it (its Scope
     names `website/docs/getting-started.md`); `story:readme-logo-and-badges` depends on that
     story, so the answer is on the site before this story starts.
  5. `ess-coverage`: find the ESS coverage figure. No page states one today (`git grep -i
     coverage -- website/docs` finds nothing). `story:readme-logo-and-badges` puts the figure in
     a README badge (`docs/badges/ess.json`), not on the site. Requires
     `story:site-shows-ess-coverage`, proposed here.
  6. `run-log-format`: find where the run log format is specified. Target
     `docs/reference/ess/loom-runlog`, answer `Run log domain`, reached from
     `docs/reference/ess` (`index.mdx:138-144`).

  A task whose target is not on the site carries `requires: <story id>`. Its bot test passes
  only when the bot ends `target missing` naming that story, so it never counts as done. The
  story that adds the target removes `requires` in its own change, and the test then holds the
  completion.
- **Gating viewports:** 1440x900 and 390x844. **Measured only:** 1280x800, 1024x768 and
  768x1024, run with `UX_ALL=1`, recorded and never failing.
- **Themes:** light and dark, the two colour modes of the site (`@beyond10x/docs-system`,
  `product-site.js:368`, dark by default). A test switches the mode as the toggle does
  (`data-theme` on `<html>` and the stored choice), so one page load serves both themes.
- **Always visible:** the navbar, the theme toggle and, on doc pages at 1440x900, the sidebar.
  At 390x844 the navbar and its menu button; the theme toggle one action later, in the opened
  menu (Docusaurus moves the navbar items and the toggle into that drawer below 997 px). Each
  element is 100 % inside the viewport, not clipped and not covered at its centre, at the top of
  the page and at the end of its content.
- **Search:** the site has none (`docusaurus.config.ts:70-79` lists no search item, and the
  docs-system wrapper adds none). The adapter says `search: absent`, and a test fails when a
  search box appears while it says so. Search enters the always-visible list with
  `story:website-local-search`.
- **Completion state:** the bot is on the target route and the answer text is wholly in the
  viewport, either in the first screen after the load or after the bot followed a link to its
  anchor (sidebar, "On this page" or an in-page link). The bot never scrolls.
- **Guidance cue:** none, so L0 and gate 6 are dropped. **Protected focus:** none.
- **Baseline file** `website/ux/baseline.json`, **round dir** `website/ux/rounds/R-nn/`,
  **findings dir** `website/ux/findings/UXF-nnn.md`.

**2. Levels.** In this story:

- **L1:** unit specs of the metric functions and of how the bot picks a link, with no page
  (`*.unit.spec.ts`, run by the same Playwright runner).
- **L2:** every sitemap route at both gating viewports in both themes. Gate 1: no horizontal
  page scroll and no element past its box; vertical page scroll and a box that scrolls inside
  itself (a code block) are allowed. Gate 2: always visible, as rule 1 lists. Gate 5: axe-core
  with the tags `wcag2a, wcag2aa, wcag21a, wcag21aa, wcag22aa` reports 0 serious or critical
  violations. One test per route loads the page once and checks the four size and theme pairs
  by resizing and switching the mode in place.
- **L3:** the screen-only bot of skill § 7, starting on the landing page `/loom/`. It uses only
  controls wholly in view and not covered, found by accessible role and name; a click or a key
  press is one action. At most 3 actions at 1440x900 and at most 5 at 390x844, where the menu
  and the "On this page" toggle cost one each. By pointer at both gating sizes; by keyboard
  alone (Tab, Enter) at 1440x900, which must complete and whose key count is compared only in a
  round.
- **The ratchet** (rule 5) and **the findings records** with their check (rule 7).

Not in this story: L4, L5, L6, L7 and L8 (§ Out of scope). Gate 3 reduces to the bot's completion
state; gate 4's "a fix in at most 2 actions" has no fix to make on a static site.

**3. Where it runs.**

- **Not in the test step.** `task check` (`Taskfile.yml:20-44`) does not call it: the step is
  bounded to 120 s and is over it today (`story:test-step-within-budget`).
- **`task website-ux`:** `npm --prefix website ci`, then `slot npm --prefix website run build`,
  the product made once per run and not counted, as the test step does not count building its
  binaries (`AGENTS.md:301-302`). Then `cargo run -q --locked -p loom-xtask -- website-ux` with
  `SLOT_TOKENS` 2, as `test-step` sets 4 (`Taskfile.yml:76-81`).
- **`loom-xtask website-ux`** picks a free port on 127.0.0.1 and has Playwright serve
  `website/build` there (`docusaurus serve --dir build --host 127.0.0.1 --port <p> --no-open`;
  the base is `/loom/`, `docusaurus.config.ts:18`). It runs
  `website/node_modules/.bin/playwright test --workers=2` with a JSON reporter, then the
  findings check and the ratchet. It prints the ten slowest tests, and it fails when the time
  from the start of Playwright to the ratchet's verdict is over 120 s. The Playwright config
  sets `timeout: 60_000` (a test over 60 s fails), `globalTimeout: 120_000` and `retries: 0`.
  The folder rule's 5 s bar per test does not apply to these browser tests.
- **Hermetic.** Every request from a page to a host other than the local server is aborted and
  recorded, and a run that recorded one fails, naming the URL. The fonts are the self-hosted
  files of `@beyond10x/docs-system` (`product-site.js:25-26`, `:62-71`), the images the site's
  own. No model call, no API key, and no network after `npm ci` and the browser install.
- **Versions.** `@playwright/test` 1.65.0 and `@axe-core/playwright` 4.14.0, exact (no `^`), in
  the `devDependencies` of `website/package.json` (`:30-36`), and locked with their
  `playwright-core` and `axe-core` in `website/package-lock.json`. These are the versions the
  site of slot runs (`~/beyond10x/slot/website/package.json:32`, `:36`). `npm ci` installs no
  browser.
- **The browser** is the Chromium headless shell of the revision Playwright 1.65.0 names, the
  same in both places, with no system-browser override. On the host it lives in
  `~/.cache/ms-playwright`. `task website-ux-setup`
  (`website/node_modules/.bin/playwright install --only-shell chromium`) installs it once, by
  hand. The lane never installs: a missing browser fails at once, naming that task. On the forge
  a step `Install the browser for the UX lane` runs
  `npx --prefix website playwright install --with-deps --only-shell chromium` before the lane, as
  slot does (`~/beyond10x/slot/.github/workflows/gate.yml:35-37`). The download is that step's,
  never the lane's.
- **On the forge** the lane is in `pages.yml` (`Documentation validation`), job `build`, which
  already builds the site on every pull request (`:48-49`), so the lane reuses that build. After
  `Bind the built site` (`:50-54`) a step `ux-changes` classifies the change, and
  `Install the browser for the UX lane` and `UX lane`
  (`cargo run -q --locked -p loom-xtask -- website-ux --main-ref HEAD^1`) run when its class is
  `full`. The checkout (`:22-24`) gains `fetch-depth: 2`, so `HEAD^1` is the base.
- **The path list** is `UX_PATHS`, a workflow-level `env:` member of `pages.yml`. It has the
  format of `GATE_PATHS` and is read by the same `.github/changed-paths.sh` of
  `story:planning-only-pull-requests-skip-build`, which the `ux-changes` step runs with
  `GATE_PATHS` set to `UX_PATHS` for that step alone. First match wins:
  - `full website/*`, `full docs/assets/*`, `full docs/badges/*`, `full README.md`;
  - `full .github/workflows/pages.yml`, `full .github/changed-paths.sh`, `full Taskfile.yml`,
    `full crates/loom-xtask/src/ux/*`;
  - `skip *`.

  A pull request whose paths are all `skip` starts no browser. `push` and `workflow_dispatch`
  always run the lane, since the script prints `full` for an event other than `pull_request`.
  `docs/` outside `assets/` and `badges/` is left out: the site reads nothing there (`git grep
  ../docs -- website` finds nothing), and it holds contracts, design notes and qualification
  records. `GATE_PATHS` decides which gate a pull request runs; `UX_PATHS` decides only whether
  this lane runs. Each is stated once, in the workflow that uses it.

**4. Language.** The Playwright specs, the probes, the bot and the L1 unit specs are TypeScript
under `website/ux/`: Playwright drives the browser from its Node runner, the site is a Node
project, and `npm run typecheck` (`website/tsconfig.json`) checks them. The lane runner, the
ratchet and the findings check are Rust, in `loom-xtask` (`crates/loom-xtask/src/ux/`), with
clap derive. `AGENTS.md:113` says "Anything that runs is Rust; command lines use clap derive",
and `loom-xtask test-step` (`crates/loom-xtask/src/test_step.rs:25`, `:42`) is already the
budgeted runner that prints its ten slowest tests. The specs write one JSON record per check
under `website/ux/out/` (git-ignored); the Rust side reads only those records and the JSON report.

**5. Ratchet.** `loom-xtask ux-ratchet [--update] [--decision <id>] [--main-ref <ref>]`, also the
last step of `website-ux`. The baseline has the shape of skill § 6:
`{about, scenarios: {<scenario>: {<key>: {value, tolerance, decision?}}}, tolerances}`.

- Scenarios are `page:<route>` and `task:<id>`.
- Keys, all lower is better: per page, summed over the gating sizes and themes,
  `layout.doc_hscroll`, `layout.past_box`, `always_visible.hidden`, `axe.serious`,
  `axe.moderate` and `axe.minor`; per task, `screen_only.actions` per gating size and
  `glance_cost.max`. Compared only in a round: `layout_shift.per_load` and
  `screen_only.keyboard_keys`, which move with the host's fonts and with every new link.
- Exit 1 on each of: a value worse than the baseline beyond its tolerance; an improvement not
  taken into the baseline in the same change; a missing value (a key with no record, or a record
  with no key); a baseline worse than the one at `--main-ref` (default `origin/main`) without a
  `decision` id. A ref that does not resolve fails, naming it. A ref that has no baseline file
  prints so and skips only that comparison.

**6. First baseline.** No number is written in advance. The first lane run with `--update` writes
`baseline.json`. The tolerances start at the skill's (counts 0, steps 5 %, `layout_shift` 0.01).
A person reads that run in `website/ux/rounds/R-01/triage.md` and decides each tolerance that
differs, with its reason. Every defect the run shows becomes a finding `UXF-nnn`:

- a defect in the pages or configuration of Loom is fixed in this story;
- a defect whose cause is in `@beyond10x/docs-system` or Docusaurus, which Loom cannot fix (its
  element carries a class of that layer), goes into `website/ux/known.json` as
  `{route, check, selector, finding}`. The gate leaves out exactly that match, the finding stays
  open with `class: input`, and the lane fails on an entry that matches nothing or names a closed
  finding, so each entry leaves when its cause is fixed.

**7. Findings.** Records `website/ux/findings/UXF-nnn.md` carry the header fields of skill § 5.
No S1 or S2 finding closes without a regression asset (a Playwright assertion or a probe check
under `website/ux/`) that exists. The check is a Rust test over fixtures and over the tree's own
findings directory, and the lane runs the same check.

## Why

Timo invoked `ui-iteration-loop` on Loom on 2026-11-22, and the UI of Loom is its docs website.
`website/package.json` has no Playwright and no axe-core (`:17-36`). The site's only checks are a
build that throws on a broken link or anchor (`docusaurus.config.ts:25-26`) and the generated-page
drift (`Taskfile.yml:185-188`, `pages.yml:48-54`). Nothing stops a page that scrolls sideways on a
phone, a toggle out of reach, a contrast failure in dark mode, or a reader who can no longer reach
an answer. 27 of the 50 pages are generated (`website/docs/reference/ess/`,
`website/docs/reference/commission/`), so a change to one generator moves many pages at once.

The lane is apart from the test step because the step's budget is 120 s and the step of Loom is
over it (`story:test-step-within-budget`); a browser lane would make it worse. The site of slot
already runs this loop (`~/beyond10x/slot/website/ux/`). This story keeps its forms and changes
two: its test timeout is 120 s (`~/beyond10x/slot/website/playwright.config.ts`), here 60 s; and
it allows a system browser (`UX_CHROME`), here one pinned browser in both places.

## ESS first

`ess/` does not change: the lane is repository tooling in `loom-xtask` and `website/ux/`, and no
product type, command or refusal is added. The first commit adds the tests below. They fail
because `website/ux/`, the `website-ux` and `ux-ratchet` commands and `UX_PATHS` do not exist.

## Acceptance

Browser tests run in `task website-ux` alone: each ends within 60 s and the lane within 120 s.
The Rust tests are modules of `crates/loom-xtask/tests/main.rs` under
`story:one-test-binary-per-crate`. Each ends under 1 s with no browser and no network, and they
run in the test step, which stays within 120 s. A `planted_` test injects its defect into a
served page of the real build and passes only when the named check reports that defect.

Commands: `slot task website-ux` runs every browser test, and `slot task website-ux -- --grep
<name>` runs one; `cargo test -p loom-xtask --locked <name>` runs a Rust test.

- Every sitemap route holds gates 1, 2 and 5 at 1440x900 and 390x844 in both themes. Test:
  `contract_<route>`, one per route.
- A table 2000 px wide with `display: table`, which the table style of Docusaurus would
  otherwise contain, makes `docs/getting-started` scroll sideways at 390x844, and gate 1 reports
  it. Test: `planted_wide_table_fails_no_scroll`.
- The theme toggle hidden in the opened menu at 390x844 makes gate 2 report `theme toggle`. Test:
  `planted_hidden_theme_toggle_fails_always_visible`.
- An `<img>` with no `alt` makes gate 5 report `image-alt` as critical, in each theme. Test:
  `planted_image_without_alt_fails_axe`.
- The links of the landing page to `docs/getting-started` hidden make the bot fail `install`
  within its count. Test: `planted_hidden_link_fails_the_bot`.
- An `<img>` with an `https://` URL is aborted, the run names its URL and fails; on the site as
  built, no request leaves the machine during the lane. Tests:
  `planted_external_request_is_refused_and_named`, `no_request_leaves_the_machine`.
- No page has a search box while the adapter says `search: absent`. Test:
  `search_is_absent_as_the_adapter_says`.
- By pointer, the bot completes `install`, `program-environment`, `release-checksum` and
  `run-log-format` within 3 actions at 1440x900 and 5 at 390x844, and by keyboard alone at
  1440x900. On `tree-approve` and `ess-coverage` it ends `target missing`, naming their
  `requires` story, while that holds. Tests: `screen_only_<task>_<size>`, `keyboard_only_<task>`.
- Glance cost is 1 for a control in the main content, 2 elsewhere in view and 3 out of view; the
  bot picks a link by its accessible name only, never by hidden text. Tests:
  `glance_cost_by_position`, `link_choice_reads_accessible_names_only`.
- The four failures of the ratchet, each on a fixture baseline and records: a count 1 worse than
  its tolerance of 0; an improved value not written back; a key with no record; a baseline worse
  than the one at a fixture git ref with no decision id (with one, exit 0). Tests:
  `ux_ratchet_fails_on_a_worse_value`, `ux_ratchet_fails_on_an_improvement_not_taken_in`,
  `ux_ratchet_fails_on_a_missing_value`, `ux_ratchet_fails_on_a_worse_baseline_without_a_decision`.
  After `--update` it exits 0 with the improvement taken in. Test:
  `ux_ratchet_update_takes_in_an_improvement`.
- A closed S1 or S2 record with no regression asset, or naming a file that does not exist, fails;
  a closed S3 with none, and a closed S2 with an asset that exists, pass. The tree's own
  `website/ux/findings/` passes. Tests: `ux_findings_refuse_a_closed_s1_or_s2_without_an_asset`,
  `ux_findings_of_the_tree_hold`.
- A `known.json` entry that matches no record, or names a closed finding, fails the lane. Test:
  `ux_known_entry_must_match_an_open_finding`.
- `UX_PATHS`, read from `pages.yml` and never copied into the test:
  `website/docs/guides/bindings.md`, `website/package-lock.json`, `docs/badges/ess.svg`,
  `docs/assets/logo.svg`, `README.md` and `.github/workflows/pages.yml` give `full`;
  `docs/design/notes.md`, `CHANGELOG.md`, `AGENTS.md`, `.engineering/planning/story/x.md` and
  `crates/loom-cli/src/lib.rs` give `skip`; the event `push` gives `full`. Tests:
  `the_ux_lane_runs_on_the_site_inputs`, `the_ux_lane_skips_text_and_code_outside_its_list`,
  `the_ux_lane_runs_on_every_push`.
- In job `build` of `pages.yml`, the browser install and the lane run only when
  `steps.ux-changes.outputs.class` is `full`, the install comes before the lane, and the lane runs
  the command `task website-ux` runs. Test: `the_pages_workflow_gates_the_ux_lane_by_class`.
- The Playwright config kills a test at 60 s and the run at 120 s, with no retries. `website-ux`
  with a stub runner that sleeps past a budget of 1 s (`LOOM_UX_BUDGET_SECONDS`, read in tests
  only) exits non-zero, naming the budget, and prints the slowest tests. Tests:
  `ux_playwright_config_bounds_each_test_and_the_run`, `ux_lane_fails_over_its_budget`.
- `task check` does not call `website-ux`. Test: `website_ux_is_not_part_of_the_check`.
- The wall time of the lane is measured at the merge, not estimated here: on the host
  (`slot task website-ux`, from the time line it prints) and on the forge run of the merging pull
  request, each at most 120 s, both written in the close note of the story. Over 120 s the story
  is not done; a raised bound, a shard or a retry is not a fix.
- The first run's `baseline.json` and `rounds/R-01/triage.md` are committed, and
  `cargo run -q --locked -p loom-xtask -- ux-ratchet` exits 0 on the merged tree.
- `AGENTS.md` § Gate (`:316-320`) and § Documentation (`:2746`) name `task website-ux`, its path
  list and its budget, and `website/docs/guides/run-the-checks.md` (`:67-71`) shows the command.
  Command: `git grep -c website-ux -- AGENTS.md website/docs/guides/run-the-checks.md
  Taskfile.yml` prints a count of at least 1 for each file.
- `slot task website` and `slot task check` exit 0.

## Out of scope

- L4 to L8, each a follow-up story proposed here and not in the store:
  `story:website-ux-keyed-replay` (L4), `story:website-ux-discoverability-probe` (L5, a vision
  model), `story:website-ux-live-rounds` (L6), `story:website-ux-review-agents` (L7, with
  `docs-site-review` in the Measure step) and `story:website-ux-real-user-sessions` (L8).
- The round command (`report.md`, `round.json`, contact sheets, `trend.md`):
  `story:website-ux-round-command`, proposed.
- A search box (`story:website-local-search`) and the ESS coverage figure on the site
  (`story:site-shows-ess-coverage`), both proposed.
- A redesign of the site, a change to `@beyond10x/docs-system`, a registry publish, any model call.
- The ruleset and the required checks, which are Timo's.

Order: after `story:test-step-within-budget`; after `story:planning-only-pull-requests-skip-build`,
whose `.github/changed-paths.sh` and `crates/loom-xtask/tests/changed_paths.rs` this story uses;
and after `story:readme-logo-and-badges`, an id inferred and not yet in the store. That story
changes `README.md`, adds `docs/assets/logo.svg` and `docs/badges/`, and points the favicon of
the site at `website/static/img/logo.svg` (`docusaurus.config.ts:11`). This story is not on the
Runbook stub-cycle path and blocks no story.

## Scope

Derived 2026-11-22 from the plan tree at `1ee45331`. **Cited** means read at the `file:line`
named. **Inferred** means a reading that could be wrong.

- **Primary surface:** `website/ux/` (new: `adapter.ts`, probes, bot, specs, `baseline.json`,
  `known.json`, `findings/`, `rounds/`) — inferred for the file names
- **Files (site):** `website/package.json` (`devDependencies`, `:30-36`),
  `website/package-lock.json`, `website/playwright.config.ts` (new); `.gitignore` (`:3-5`) gains
  `/website/ux/out/` — cited for the existing files, inferred for the config's place
- **Files (tooling):** `crates/loom-xtask/src/main.rs` (the `Command` enum, `:88`, gains
  `website-ux` and `ux-ratchet`); `crates/loom-xtask/src/ux/` (new), shaped like
  `crates/loom-xtask/src/test_step.rs` (`:25`, `:42`) — cited for the enum and `test_step.rs`,
  inferred for the module
- **Files (tests):** `crates/loom-xtask/tests/` (the `ux_` modules); `changed_paths.rs` gains the
  `UX_PATHS` cases — inferred: `changed_paths.rs` is not on the tree yet
- **Files (tasks):** `Taskfile.yml`: new `website-ux` and `website-ux-setup` beside `website`
  (`:190-195`); `check` (`:20-44`) unchanged — cited
- **Files (CI):** `.github/workflows/pages.yml`: `env:` gains `UX_PATHS`, the checkout
  (`:22-24`) gains `fetch-depth: 2`, three steps follow `:50-54` — cited
- **Files (docs):** `AGENTS.md` § Gate (`:316-320`) and § Documentation (`:2746`);
  `website/docs/guides/run-the-checks.md` (`:67-71`) — cited
- **Also likely:** pages or `website/` configuration the first run finds a defect in — inferred
- **Not changed:** `.github/workflows/check.yml`, `ess/`, `generated/`,
  `@beyond10x/docs-system` — cited
- **Confidence:** medium — the existing surfaces are read; the new module and file names, the
  fixes of the first run and `changed_paths.rs` (not on the tree) are not
- **Would collide with:** `story:planning-only-pull-requests-skip-build` on
  `.github/changed-paths.sh` and `changed_paths.rs`; `story:readme-logo-and-badges` on
  `Taskfile.yml`, `crates/loom-xtask/src/main.rs`, `docusaurus.config.ts` and `AGENTS.md`;
  `story:test-step-within-budget` on `Taskfile.yml` and `crates/loom-xtask`; each lands first.
  Any unit that changes a page under `website/docs/` moves the baseline — inferred for the order
- **Safety fact:** with its last rule `skip *`, `UX_PATHS` lets a pull request skip the lane only
  when no path the site is built from changed, because the site reads nothing under `docs/`
  outside `assets/` and `badges/` (`git grep ../docs -- website` finds nothing at `1ee45331`) —
  step 2, unproven; a page or plugin that starts reading `docs/` needs a rule in the list
