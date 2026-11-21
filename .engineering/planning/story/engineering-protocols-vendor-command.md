---
format: aep.planning-md/3
id: story:engineering-protocols-vendor-command
kind: story
status: active
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
  path: crates/loom-xtask/tests/protocols_vendor.rs
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:18:31Z", actor: "human:timo", revision: 10}
- {from: "proposed", to: "active", at: "2026-11-21T07:20:40Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
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

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-xtask` (a new `protocols_vendor` module and subcommand) — cited (story § Files; `crates/loom-xtask/src/main.rs:80` is the `Command` enum)
- **Files:** `crates/loom-xtask/src/main.rs` — cited (new `Command` variant at `:80`, a `run` match arm in the `match` at `:170-210`, `mod protocols_vendor;` beside `mod release;`)
- **Files:** `crates/loom-xtask/src/protocols_vendor.rs` (new) — cited
- **Files:** `crates/loom-xtask/tests/protocols_vendor.rs` (new) — cited
- **Files:** `crates/loom-xtask/Cargo.toml` — cited (`b10x-loom-protocols` by path, after `:11`)
- **Files:** `Cargo.lock`, `AGENTS.md` (§ Protocol composition and system queries) — cited (house-rule files, left out of the `paths` block)
- **Symbols:** `loom_protocols::lock::{REPOSITORY, locked_file, encode, decode}`, `ProtocolCatalog::from_vendored` — cited (story § Outcome; all declared by `story:engineering-protocol-lock`, none exist in the tree today)
- **Symbols:** `Command` (`crates/loom-xtask/src/main.rs:80`), `run` (`:165`), `find_root` (`:214`), `Cli.root` (`:71`) — cited
- **Files:** `crates/loom-intake-slice/tests/host_git_hardening.rs` — cited (revision e5r3: `protocols_vendor.rs` is added to its `helpers` list, `:479-481`, because the module has its own byte-returning git helper)
- **Not touched:** `crates/loom-xtask/src/release.rs` — cited (revision e5r3: `release::git` is not reused, since it decodes lossily)
- **Documents:** `AGENTS.md` § Protocol composition and system queries — cited
- **Confidence:** high for the five named files, because the story lists them. Medium for the whole change, because the story does not say how the module starts `git`, and the tree's rule on that is unmet by the story as written.
- **Would collide with:**
  - `crates/loom-xtask/src/main.rs`: `story:consumer-sdk-paths` (draft, `consumer-paths` subcommand, its scope names `main.rs`) — cited. No `depends_on` path orders the two: `consumer-sdk-paths` has only `decomposes`/`serves` edges. Both add a `Command` variant, a `run` arm and a `mod` line, so they conflict.
  - `crates/loom-xtask/src/release.rs`: no longer touched (revision e5r3), so no collision with `story:command-invocation-contract`, `story:co-pinned-consumer-check` or `story:generated-types-release-surface` there.
  - `crates/loom-xtask/Cargo.toml`: no other open story names it — inferred (grep of the planning store: only this story and the implemented `loom-agent-resource-domain`).
  - `Cargo.lock`: `story:engineering-protocols-dependency-removed` — cited. Ordered: it `depends_on` this story (graph). `story:canon-0-9-0-pin` also moves `Cargo.lock`. It is ordered before this story through `engineering-protocol-lock depends_on canon-0-9-0-pin`, and this story `depends_on` the lock.
  - `AGENTS.md` § Protocol composition and system queries: `story:engineering-protocols-dependency-removed` edits § Boundary, § Governor and § Connectors, not the same section, and is ordered after this story. `story:engineering-protocol-lock` edits `AGENTS.md`, and this story `depends_on` it (ordered).
  - `crates/loom-xtask/tests/` (siblings, not the same file): `story:engineering-registry-calls-replaced` edits `tests/release_check.rs`. It has no edge to this story and no shared file, so it does not collide.
- **Safety fact:** the new git start is safe only if it goes through a helper `host_git_hardening.rs` accepts. `every_host_git_command_goes_through_a_hardened_helper` scans `crates/*/src` for `Command::new(` with `git` in the argument (`crates/loom-intake-slice/tests/host_git_hardening.rs:441-478`), allows exactly three helper files each starting git once (`:480-495`), and requires `core.hooksPath=` and `core.fsmonitor=false` in each (`:497-504`) — step 2 (file:line read), unproven
- **Safety fact:** adding `b10x-loom-protocols` to `loom-xtask` introduces no dependency-edge check. `crate_names.rs` lists `("loom-xtask", "loom-xtask", None)` at `:119` and checks names only, and nothing in `crates/*/tests` scans `loom-xtask`'s manifest — step 2 (`git grep`), unproven. The same grep did not read `pinned_consumer.rs`, which runs `cargo tree --duplicates` on `consumers/pinned` and not on the workspace.
