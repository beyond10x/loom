---
format: aep.planning-md/3
id: story:breaks-between-releases
kind: story
status: draft
title: A consumer moving between two Loom tags reads every break between them on one generated page and follows one rule
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A consumer moving its pin from one Loom tag to another reads every break between them on one
page, and follows one written rule to move. `loom-docs` generates
`website/docs/reference/breaking-changes.md` from `CHANGELOG.md`: per release, newest first, each
`Breaking:` sentence of its entry as written. `website/docs/guides/move-to-loom.md` gains a section
"Move between Loom releases" that states the rule: change the tag, apply each break the page names
for every release after the old tag up to and including the new one, build and run; a break the page
does not name is a Loom defect, reported against the release, and never patched around in Loom.

## Why

The epic's third acceptance asks that a consumer move to a release without patching Loom, and
`story:release-names-embedder-breaks` makes every entry name its breaks. Reading them is still the
consumer's work: the sentences are spread over twenty-one entries in three forms (`Breaking:`,
`- Breaking:`, `**Breaking:**`), inside Added, Changed and Fixed lists, and nothing tells a consumer
crossing several releases which ones apply. control-plane, the engineering factory's integration
point (served first; cleared `decision-blocker:g6-first-consumer`), moved from a commit before
0.10.0 to the 0.20.0 tag; a consumer crossing releases so finds its breaks only by reading every
entry in between.
`move-to-loom.md` covers moving from the archived repositories and from Harness, not between
Loom releases.

## Acceptance

`task docs-check` fails when the generated page and the `Breaking:` sentences of `CHANGELOG.md`
disagree, and the guide states the rule:

- `crates/loom-docs/src/breaking.rs::every_breaking_sentence_of_the_changelog_is_on_the_page` reads
  every released entry of `CHANGELOG.md` (not **Unreleased**), takes its `Breaking:` sentences in the
  three forms and across line breaks, and asserts the page lists each once under its release, with
  `Breaking: none.` shown as "No break named." and an entry with no sentence (those before the rule)
  as "No `Breaking:` sentence recorded."
- `crates/loom-docs/src/breaking.rs::the_page_orders_releases_newest_first` and
  `a_sentence_spanning_lines_is_one_item` hold the generator over hand-built changelogs.
- `loom-docs generate` writes the page with the generated header; `loom-docs generate --check` (in
  `task docs-check`) reports a missing, stale or hand-edited page.
- `move-to-loom.md` has the "Move between Loom releases" section with the rule above, linking the
  generated page, `reference/sdk-stability.md` and `reference/run-events.md`.
- `task check` and `task website` exit 0.

## ESS first

No behaviour change: a generated documentation page from an existing file and a guide section. No
type, command, port or outcome changes, and no noun is introduced, so `ess/` is untouched. The red
step is `every_breaking_sentence_of_the_changelog_is_on_the_page`, which fails before the page
exists.

## Domain relations

None. A release entry and a break sentence are not nouns of any `ess/1` document in this
repository, and the story introduces none.

## Not in scope

- Rewriting or reformatting earlier CHANGELOG entries; the page shows each sentence as written.
- Changing what `release-check` refuses (`story:release-names-embedder-breaks`).
- Deciding whether a change is a break, or how a consumer applies one in its own code.
- Any change in control-plane or another consumer's repository; the agent platform (consumer A),
  which waits.

## Scope

Crates: `loom-docs`.

- `crates/loom-docs/src/breaking.rs` (new), `crates/loom-docs/src/main.rs`
- `website/docs/reference/breaking-changes.md` (new, written by `loom-docs generate`, never
  hand-edited)
- `website/docs/guides/move-to-loom.md`: the "Move between Loom releases" section; frontmatter
  `description`, `lede` and `source` widened
- `AGENTS.md`: the page in § Generated files and a row in § Checks that hold the rules;
  `CHANGELOG.md` (**Unreleased**)

Touches none of `ess/`, `generated/`, `crates/loom-xtask`, `crates/loom-intake-slice`,
`crates/loom-governor`, `crates/loom-cli`, `Cargo.toml`, `Cargo.lock`.
`crates/loom-docs/src/main.rs` is also in `story:command-invocation-contract`'s scope (one generator
each); `AGENTS.md`, `CHANGELOG.md` and `move-to-loom.md` are shared prose; merge them.

## Ready

Ready now; it needs no answer from anybody.
