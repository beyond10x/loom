---
format: aep.planning-md/3
id: story:engineering-protocols-vendor-command
kind: story
status: implemented
title: loom-xtask protocols-vendor rewrites the vendored files and the lock from a local engineering-protocols checkout at a tag
summary: Reads Git objects only, never fetches; writes the lock through loom_protocols::lock with the constant repository; tested on a temporary repository whose output loads through from_vendored
tags:
- sdc
relations:
- decomposes: epic:engineering-protocols-as-vendored-data
- serves: vision:O1
- depends_on: story:engineering-protocol-lock
scope:
- confidence: cited
  path: crates/loom-intake-slice/tests/host_git_hardening.rs
- confidence: cited
  path: crates/loom-xtask/Cargo.toml
- confidence: cited
  path: crates/loom-xtask/src/main.rs
- confidence: cited
  path: crates/loom-xtask/src/protocols_vendor.rs
- confidence: cited
  path: crates/loom-xtask/tests/adversary_w20261121w4_u4.rs
- confidence: cited
  path: crates/loom-xtask/tests/adversary_w20261121w4_u4_pass2.rs
- confidence: cited
  path: crates/loom-xtask/tests/protocols_vendor.rs
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:18:31Z", actor: "human:timo", revision: 10}
- {from: "proposed", to: "active", at: "2026-11-21T07:20:40Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
- {from: "active", to: "implemented", at: "2026-11-21T09:11:56Z", actor: "agent:sdc", revision: 15, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
---
## Outcome

`cargo run -q --locked -p loom-xtask -- protocols-vendor --checkout <dir> --tag <tag>` rewrites
`protocols/engineering/` and its `lock.json` under the repository root (the global `--root`, else
found as the other commands find it: `run`, `crates/loom-xtask/src/main.rs:166-169`, and
`find_root`, `:214-225`, the nearest ancestor holding `ess/ess-inputs.yaml`) from a local Git
checkout of engineering-protocols at that tag. It resolves the tag to its commit in the checkout,
reads every `protocols/<name>/<major>.yaml` blob of that commit, writes each byte for byte at
`<name>/<major>.yaml`, removes a vendored file the tag no longer holds, and writes the lock through
`story:engineering-protocol-lock`'s module: `lock::locked_file` per file and `lock::encode` for the
bytes, so the command and the catalog's load share one encoder and one decoder.

Byte for byte means exactly: each written file's bytes equal the blob's bytes, whatever they are,
and the lock's sha256 is taken over those bytes; no step decodes a blob as text. The module starts
`git` through its own hardened helper, a single `Command::new("git")` in
`crates/loom-xtask/src/protocols_vendor.rs` that returns standard output as `Vec<u8>`, with the
settings `release::git` uses (`crates/loom-xtask/src/release.rs:636-655`: `-c
core.hooksPath=/dev/null`, `-c core.fsmonitor=false`, `GIT_DIR`, `GIT_WORK_TREE` and
`GIT_INDEX_FILE` removed, `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`,
`GIT_TERMINAL_PROMPT=0`, standard input null). It does not reuse `release::git`, which returns
`String::from_utf8_lossy` of the output and so alters a byte that is not UTF-8. Blobs are read with
`git cat-file blob <commit>:<path>`. `every_host_git_command_goes_through_a_hardened_helper`
(`crates/loom-intake-slice/tests/host_git_hardening.rs:441-504`) allows exactly three files to
start `git` (`helpers`, `:479-481`), each once and each setting `core.hooksPath=` and
`core.fsmonitor=false`; this story adds `crates/loom-xtask/src/protocols_vendor.rs` to that list.

The lock's members come from two places only:

- `repository` is the constant `lock::REPOSITORY`
  (`https://github.com/beyond10x/engineering-protocols`), never read from the checkout's remote,
  which may be SSH, HTTPS or absent (`~/beyond10x/engineering-protocols` has an SSH `origin`);
- `tag` is the argument, and the commit and every file's bytes come from the checkout's Git
  objects.

It reads Git objects only: it never fetches, so the maintainer fetches the tag first. It validates
everything before it writes, so a refusal changes nothing under the root. Run twice on the same tag
it writes the same bytes. It does not compile the documents with Canon; the catalog's load and
`task check` do.

`crates/loom-xtask/Cargo.toml` gains `b10x-loom-protocols = { path = "../loom-protocols" }` and no
sha256 crate. Nothing forbids it: `loom-xtask` already depends on the product crate
`b10x-loom-resource` (`crates/loom-xtask/Cargo.toml:11`), which depends on a generated crate the
command `generate` rewrites, as `b10x-loom-protocols` depends on the generated intake crate
`intake-generate` rewrites; `crates/loom-executor/tests/crate_names.rs` checks package and library
names, not dependency edges; and `AGENTS.md` § Rules restricts only Commission core's
dependencies (committed `:96-97`).

## Why

`story:engineering-protocol-lock` copies the files once by hand. A later tag needs a command that
cannot leave the files and the lock disagreeing, and that runs in a test with no network call
(`AGENTS.md` § Never). `loom-xtask` already holds the repository's maintenance commands (`drift`,
`crd-generate`, `release-check`: the `Command` enum, `crates/loom-xtask/src/main.rs:80`) and finds
the root as they do.

## Waits on

`story:engineering-protocol-lock`: the directory layout and files the test reads, the `lock`
module and `ProtocolCatalog::from_vendored`.

`story:engineering-protocols-dependency-removed` waits on this story: both edit `Cargo.lock` (this
story adds `b10x-loom-protocols` to the dependency list of the `loom-xtask` package) and
`AGENTS.md` (this story § Protocol composition and system queries; that story § Boundary,
§ Governor and § Connectors, below the lines this story inserts).

## ESS first

Exempt from an `ess/` change: the lock it writes is `intake.protocols.ProtocolLock`, declared by
`story:engineering-protocol-lock`, and the command adds no noun. The red step is
`crates/loom-xtask/tests/protocols_vendor.rs`, which fails while the subcommand is unknown.

## Acceptance

`crates/loom-xtask/tests/protocols_vendor.rs` builds a Git repository in a temporary directory with
an `origin` remote `git@example.invalid:someone/fork.git` and three tagged commits. The protocol
files of `t1` and `t2` are the tree's own vendored files, read from
`Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocols/engineering")` (written by
`story:engineering-protocol-lock`), so Canon accepts them: tag `t1` holds
`protocols/software-change/1.yaml`, `protocols/incident-response/1.yaml` and a
`protocols/vocabulary.yaml`; tag `t2` drops `incident-response/1.yaml` and adds
`support-triage/1.yaml`; tag `t3` holds only `protocols/raw-bytes/1.yaml`, a fixture file whose
bytes include `0xff` and `0xfe` (not UTF-8). Each case runs the command with `--root` at a
temporary root, which is empty unless the case says it starts from an earlier run's output:

- at `t1` the root's `protocols/engineering/` holds exactly `software-change/1.yaml` and
  `incident-response/1.yaml`, byte for byte, and `lock.json`; `vocabulary.yaml` is not copied;
- at `t1` `ProtocolCatalog::from_vendored` over the files and `lock.json` the command wrote builds a
  catalog of exactly `software-change@1` and `incident-response@1`, each with revision `t1`;
- at `t1` `lock::decode` of `lock.json` gives `repository` equal to `lock::REPOSITORY`, though the
  checkout's `origin` is another URL, and the commit `git rev-parse t1^{commit}` prints;
- at `t1` `lock.json` equals `lock::encode` of what it decodes to, byte for byte;
- a second run at `t1` on the root the first run wrote leaves every byte under the root unchanged;
- a run at `t2` on the root a `t1` run wrote leaves no `incident-response/1.yaml` (nor its
  `incident-response/` directory) under `protocols/engineering/`;
- that `t2` run leaves exactly `software-change/1.yaml` and `support-triage/1.yaml` and
  `lock.json`, and `from_vendored` over them builds exactly `software-change@1` and
  `support-triage@1`, each with revision `t2`;
- at `t3` the written `raw-bytes/1.yaml` equals the fixture's bytes, `0xff` and `0xfe` included,
  and its lock entry's `sha256` is the sha256 of those bytes;
- a tag the checkout does not hold is refused naming the tag, and nothing under the root changes;
- a `--checkout` that is a URL rather than a directory is refused naming the argument, and nothing
  under the root changes;
- a tag holding `protocols/software-change/one.yaml` is refused naming that path, and nothing under
  the root changes.

Beside the test:

- `every_host_git_command_goes_through_a_hardened_helper`
  (`crates/loom-intake-slice/tests/host_git_hardening.rs`) lists
  `crates/loom-xtask/src/protocols_vendor.rs` among its helpers and passes.
- Run on the real checkout at `0.16.0`, the command leaves `git status --porcelain
  protocols/engineering` empty (by hand, as no test reads a checkout outside the tree; recorded in
  the story's evidence).
- `AGENTS.md` § Protocol composition and system queries says how the vendored catalog moves: fetch
  the tag into a checkout, run the command, run `task check`; the added text names neither
  `b10x-canon-engineering` nor a tag (`git grep -n -e 'b10x-canon-engineering' -e '0\.16\.0' --
  AGENTS.md` prints no line of that section).
- `task check` exits 0.

## Files

`crates/loom-xtask/src/main.rs`, `crates/loom-xtask/src/protocols_vendor.rs` (new, with its own
git helper), `crates/loom-xtask/tests/protocols_vendor.rs` (new),
`crates/loom-intake-slice/tests/host_git_hardening.rs` (one more entry in `helpers`),
`crates/loom-xtask/Cargo.toml` (`b10x-loom-protocols` by path), `Cargo.lock` (the `loom-xtask`
package's dependency list; shared with `story:engineering-protocols-dependency-removed`, which is
ordered after this story), `AGENTS.md` (§ Protocol composition and system queries; shared with the
same story). Not `crates/loom-xtask/src/release.rs`. No `CHANGELOG.md` bullet: `loom-xtask` is
`publish = false` and no consumer sees the command.

## Out of scope

Fetching from a remote. Checking documents with Canon in the command. Reading the upstream
catalog manifest when it exists. Vendoring any repository other than engineering-protocols. Any
change to `crates/loom-protocols` (the `lock` module and `from_vendored` are
`story:engineering-protocol-lock`'s) or to `release::git`. A `Taskfile.yml` entry for the command:
it runs by hand on a new tag, and its test runs in `task check`.

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w4 from the unit's commits (`git diff --stat 63668a97 fdc4e87a`: 9 files, 2178 insertions, 3 deletions). Every line is cited from that diff.

- **Primary surface:** `crates/loom-xtask` — cited, 6 of the 9 changed files are under it (`Cargo.toml`, `src/main.rs`, the new `src/protocols_vendor.rs`, three new test files)
- **Files (ESS sources, generated output):** none — cited; no file under `ess/`, `generated/` or `protocols/` is in the diff
- **Files (manifests):** `crates/loom-xtask/Cargo.toml` (1 line: `b10x-loom-protocols` by path, no sha256 crate), `Cargo.lock` (1 line: `b10x-loom-protocols` in the `loom-xtask` dependency list) — cited
- **Files:** `crates/loom-xtask/src/protocols_vendor.rs` — cited, new, 387 lines: the `protocols-vendor` command, its own byte-returning `git` helper, a Git 2.44.0 gate, and 3 unit tests
- **Files:** `crates/loom-xtask/src/main.rs` — cited, 20 lines: `mod protocols_vendor;`, a `Command::ProtocolsVendor { checkout, tag }` variant with its help text, its `run` arm, and a paragraph in the crate doc
- **Files (new tests):** `crates/loom-xtask/tests/protocols_vendor.rs` (788 lines, 18 cases, the story's temporary-repository test), `crates/loom-xtask/tests/adversary_w20261121w4_u4.rs` (500 lines, 9 cases) and `crates/loom-xtask/tests/adversary_w20261121w4_u4_pass2.rs` (445 lines, 7 cases), the two adversary passes' cases, committed unchanged
- **Files (existing tests):** `crates/loom-intake-slice/tests/host_git_hardening.rs` — cited, 29 lines (28 added, 1 deleted): `protocols_vendor.rs` joins `helpers` in `every_host_git_command_goes_through_a_hardened_helper`, and the test also requires the vendor helper to hold `protocol.ext.allow=never`, `protocol.allow=never`, `--no-replace-objects`, `GIT_NO_LAZY_FETCH=1` and `GIT_CONFIG_GLOBAL=/dev/null`
- **Files (documents):** `AGENTS.md` (10 lines: § Protocol composition and system queries gives the three steps, fetch the tag, run `protocols-vendor`, run `task check`, and the Git 2.44.0 minimum) — cited
- **Symbols:** public item added: none; `loom-xtask` is a binary crate. Crate-private: `protocols_vendor::protocols_vendor(root, checkout, tag)` (`pub(crate)`), the `Command::ProtocolsVendor` variant with its two `--checkout` and `--tag` arguments (the `protocols-vendor` subcommand), and private helpers `lock_of`, `refusal`, `destination`, `supported_git`, `commit`, `protocol_paths`, `write`, `git` and the constants `VENDORED`, `UPSTREAM`, `MINIMUM_GIT`. No symbol changed or removed — cited
- **Not predicted and touched:** `crates/loom-xtask/tests/adversary_w20261121w4_u4.rs`, `crates/loom-xtask/tests/adversary_w20261121w4_u4_pass2.rs` (the adversary passes' cases). `Cargo.lock` and `AGENTS.md` are in the stored Files and Scope text but not in the typed scope — cited
- **Predicted and not touched:** none — cited; all 5 files of the typed scope are in the diff, and `crates/loom-xtask/src/release.rs` is untouched as the story says
- **Review:** `review-result:engineering-protocols-vendor-command-adversary-1` (head `8fc87502`): red, 89 cases executed, 1 red, 9 findings introduced: a branch named `refs/tags/<tag>` was vendored as the tag (fixed in the diff: the tag is resolved with `show-ref`), seven guards without a case, now covered, and one wrong doc comment on transports. `review-result:engineering-protocols-vendor-command-adversary-2` (head `9dd940d7`): green, 99 cases executed, 0 red, 4 notes introduced: the doc comment at `protocols_vendor.rs:293-295` claims too much, `AGENTS.md` did not name Git 2.44.0 (the diff's text does), the hardening check matches setting names without values (the diff's version matches `GIT_NO_LAZY_FETCH=1` and `GIT_CONFIG_GLOBAL=/dev/null` with values), and a tag pointing at a tree is refused with the missing-tag message. No security review recorded.
