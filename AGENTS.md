# AGENTS.md — loom

What Loom is and how to build it is in [README.md](README.md); this file is what an agent changing it
must know. The cross-repository architecture is Atlas ADRs 0066–0075 (0090 made Loom the one runtime
repository, with Commission, the governor and intake) and Atlas
`docs/design/governed-autonomy/`.

## Serves

- **O1 — governed reach.** The model only sees and can only execute actions the current frontier
  admits; authority is rechecked before every effect.
- **O3 — any harness, observed and compared.** Loom is the native executor Metaharness compares
  against other harnesses on the same case.

## Boundary

- Loom owns the model/tool loop (Atlas ADR 0071): prompt and context assembly, model turns, the
  projected action catalogue, action selection, argument generation, tool round trips, streaming,
  compaction, sessions and transcripts, budgets, interruption and recovery, model-facing approval
  suspension.
- Loom does not own protocol semantics, engineering semantics, case truth, organizational authority,
  connector credentials, final completion or system conformance semantics.
- Loom depends on Commission's core contracts and implements `AgentExecutor`. Commission core never
  depends on Loom (Atlas ADR 0075).
- `beyond10x/harness` is the predecessor. Port its implementation; do not rewrite from zero, and do
  not break its current consumers while they still depend on it. Harness is
  `LicenseRef-B10x-Proprietary`; code ported from it into Loom is relicensed Apache-2.0 (operator,
  2026-10-04). Harness itself keeps its licence.

## Rules

- Model-visible actions are derived from the current frontier and runtime capability; nothing
  consequential is granted because it was registered at startup (Atlas ADR 0072).
- A selector picks only from the candidate set it was given; unknown action ids are rejected;
  confidence never grants authority; low confidence falls back to a stronger path (Atlas ADR 0073).
- Revalidate every selected action against case revision, frontier and authority before execution.
- A trace is not evidence (Atlas ADR 0074).
- The model is not trusted context: it never supplies identity, authority, case revision, trusted
  time, approval results or tenant context.
- When a selector, integration, verifier or authority provider fails, fail toward less authority,
  less effect and more explicit uncertainty. Never silently broaden capability.
- Do not make Loom semantically dependent on one selector vendor or model.
- Anything that runs is Rust; command lines use clap derive.

## ESS

Loom is specified in ESS under `ess/`. The domain is drafted and validated before any story
introduces a noun, and `task check` runs its conformance suite once synthesized. Change the
specification first.

The specification is a hard gate (Atlas ADR 0076). `task check` enforces it through
`task ess-gate`, which runs `crates/loom-executor/tests/ess_gate.rs`; a failure of any of the four
conditions fails `task check`:

1. `ess specify validate --path ess --strict-requires` exits 0;
2. `ess specify compile --path ess --format json` exits 0;
3. `ess verify conform synthesize` exits 0 with 0 refusals;
4. no file under `ess/` contains `UNMAPPED:` — the specification carries no open question.

No story is implemented while the gate is red, whether or not it edits `ess/`. The `UNMAPPED:`
scan stands in for ESS until ESS refuses open entries itself: the `UNMAPPED:` scan is removed when
the ESS release that refuses open entries (beyond10x/ess `epic:typed-open-questions`) is pinned.

Commission's specification is its own ESS system under `ess/commission/` (`system.yaml`,
`ess-inputs.yaml`), held to the same four conditions with `--path ess/commission` by
`task commission:ess-gate` (`crates/loom-commission/tests/ess_gate.rs`), which `task check` also runs.

An open question is settled before the specification changes — in a story, or in a
`decision-blocker` when nobody has decided it — and is never written into `ess/` as an
`UNMAPPED:` marker.

Spec first, then red, then implement (Atlas ADR 0080). A unit's first commit changes only `ess/`;
on it a named test fails (a conformance scenario, `task drift`, or the story's own new test when
its declarations already landed), and the run is recorded; later commits make it pass without
changing `ess/`. Every story names that change and that test in its `## ESS first` section. Only a
change with no behaviour change is exempt, and its story says so.

## Work

- Planned in the AEP store under `.engineering/`, written only through `aep plan artifact`. Body
  drafts go in `.engineering/drafts/` (ignored).
- Build with `CARGO_TARGET_DIR=$HOME/.cache/b10x-target/loom` (the Taskfile sets it; a
  `CARGO_TARGET_DIR` already in the environment wins, so a worktree can keep its own).
- `generated/rust/loom/` is ess output, byte-pinned by `task drift`; it is never formatted.
  `generated/rustfmt.toml` sets `disable_all_formatting`, so `cargo fmt --all` (which reaches the
  path dependency) leaves it alone. Do not add a `rustfmt.toml` inside the generated tree: `task
  drift` would report it and `task generate` would delete it.
- `b10x-loom-commission` (and, for tests, `b10x-loom-commission-testkit`) is a path dependency on
  `crates/loom-commission` (`crates/loom-commission-testkit`) in this workspace. The frontier Loom reads is
  Commission's generated `Frontier`; Loom may use `b10x-canon`, the Commission contracts crate may
  not.
- Every commit and push is `b10x-bot[bot]`'s through `b10x-gates bot`; every GitHub write goes
  through `b10x-gates api`.
- Use a managed worktree (`worktree create --repo loom --purpose …`) for changes.

## Documentation

The site (`website/`, Docusaurus on `@beyond10x/docs-system`) is written with the workspace `docs`
skill (`~/beyond10x/.agents/skills/docs/SKILL.md`). It is independent: `pages.yml` (`Documentation
validation`) builds it and `b10x-docs-site.yml` (`Documentation site`) deploys it to
<https://beyond10x.github.io/loom/>.

- Generated, never edited by hand: `website/docs/reference/cli.md` (from `b10x_loom_cli::Cli` in
  `crates/loom-cli/src/lib.rs`), `website/docs/reference/crates.md` (from `cargo metadata`; every
  package needs a `description`, and it is public text, so no story ids), and
  `website/docs/reference/ess/` with `website/data/ess/` (from `ess/` and `ess/intake/`), all by
  `loom-docs` (`task docs-generate`, checked by `task docs-check`); `website/docs/reference/commission/`
  by `loom-commission-docs` (`task commission:docs`, checked by `task commission:docs-drift`).
- `website/data/status.json` is maintained by hand. A change that ships, decides or drops a
  capability updates it, the page that describes it and `CHANGELOG.md` (**Unreleased**) in the same
  commit; a change to a command, a crate or a rule updates `README.md` and this file too.
- Every command on a page is run in the tree before it is written, and its output is pasted from
  that run. Never make a live model call for a page: cite a record under `docs/qualification/`.
- `task website` builds the site as CI does (broken links and anchors throw).

## Commission

Commission's history was merged into Loom; its files keep their Commission paths so `git log
--follow` reaches it. Crates: `crates/loom-commission` (`b10x-loom-commission`, the contracts; its dependency
tree names no `b10x-canon`, `b10x-loom-executor` or model-provider crate, enforced by `task
commission:deps-guard` and `crates/loom-commission-testkit/tests/skeleton.rs`), `crates/loom-commission-testkit`,
`crates/loom-commission-conformance`, `crates/loom-commission-docs` and `crates/loom-commission-xtask`. ESS:
`ess/commission/`, generated into `generated/rust/commission/` (`task commission:generate`, pinned by
`task commission:drift`). Docs: `docs/commission/`, reference pages in
`website/docs/reference/commission/` (`task commission:docs`). Its tasks live in
`Taskfile.commission.yml`, included under the `commission:` namespace.

## Governor

`crates/loom-governor` (`b10x-loom-governor`, lib `loom_governor`) puts Canon behind the governor and evidence
ports Commission defines (Atlas ADR 0089); its history was merged from `beyond10x/governor` and it is
a Loom crate under Atlas ADR 0090. It decides and never acts: it evaluates a case's protocol and
reports the frontier and completion, and executes nothing. It is the only crate here that evaluates
protocols with Canon today. It depends on Commission by path, never the reverse.

- Canon is named by the reference `b10x-canon-engineering` uses (`branch = "main"`), pinned by
  `Cargo.lock`: a different reference builds a second Canon whose types do not match its. Move
  Canon with `cargo update -p b10x-canon` together with the `b10x-canon-engineering` tag
  (beyond10x/engineering-protocols). Any other Loom crate that adds Canon uses the same reference.
- The governor adds no clock, network or model call to an evaluation.
- ESS: the governor has no domain of its own. Its nouns are Commission's (`CaseId`, `Frontier`,
  `Evidence`, `CompletionDetermination`) and Canon's (case snapshot, decision; Canon opts out in
  favour of its own conformance). A noun it introduces gets an `ess/` domain before a story is
  written around it.

## Intake

Intake's history was merged from `beyond10x/intake`; it routes an intent to a proposed protocol and
runs a small vertical slice over it. It builds against Loom's own `b10x-loom-commission`,
`b10x-loom-governor` and `b10x-loom-executor` by path. ESS: the `intake.routing` domain under `ess/intake/`
(`task intake-spec`, which `task check` runs). Docs: `docs/intake/`.

- `crates/loom-intake-router` (`b10x-loom-intake-router`): classifies an intent against the
  engineering protocol registry (`b10x-canon-engineering`, beyond10x/engineering-protocols); a
  pick outside the registry or below the confidence threshold is refused.
- `crates/loom-intake-references` (`b10x-loom-intake-references`): extracts tracker keys, chat permalinks,
  merge and pull requests and URLs from an intent, deterministically.
- `crates/loom-intake-slice` (`b10x-loom-intake-slice`): the local effect adapter (`LocalEffects` over the
  local executor) and a thin caller of Commission's runtime (`run_until_blocked`); it has no loop
  of its own (`story:runtime-merge`). Keep it small and do not grow it into a runtime.
- `crates/loom-cli` (`b10x-loom-cli`): the `b10x-loom` command line (`b10x-loom run`, which
  replaced `b10x-intake run` in `story:loom-cli`). It took its `loom-` name in
  `story:crate-names`. Its clap definition is the library (`src/lib.rs`, `Cli`), which `loom-docs`
  renders into the CLI reference; a flag change regenerates it (`task docs-generate`).

Rules that still hold:

- A routing proposal is never authority; whoever opens the case checks it.
- Intake reads the engineering protocol registry and calls Canon, Commission, the governor and
  Loom; it re-implements none of them, and never evaluates Canon itself.
- The slice executes `software.change/1` actions inside the given workspace only. It never merges,
  pushes or deploys, and never supplies authority on the operator's behalf.
- There is no sandbox. `tests.run` and the workspace's git hooks run model-edited code with the
  operator's rights and environment; the path checks bound what the executor writes, not what that
  code does. Run the slice only on a workspace whose test command you would run yourself.
- Evidence comes from a trusted verifier (the slice runs the test command itself), never from what a
  model says happened (Atlas ADR 0074).
- Model calls go through the `llm` crates (`b10x-llm-*` at a pinned tag), never a hand-written HTTP
  client. The Codex preset and the one forced tool call are llm's `b10x-llm-tool-call`
  (`codex_model`, `call_tool`); the router and the CLI call it directly. The credential is the operator's Codex subscription (`~/.codex/auth.json`, refreshed
  through `auth.openai.com`), read and renewed by llm's credential, never by intake code; tests
  read no credential file.
- A model's choice is checked against the list it was given; a pick outside it is refused.
- Tests make no model or network call: they use recorded responses and local fixtures.
- No `/home/<name>/` path literals anywhere: common Gates personal-paths has no allowance.
