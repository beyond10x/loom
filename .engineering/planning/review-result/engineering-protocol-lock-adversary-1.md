---
format: aep.planning-md/3
id: review-result:engineering-protocol-lock-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on story:engineering-protocol-lock (wave 2026-11-21-w3, unit U3, head 3e768c32)
relations:
- reviews: story:engineering-protocol-lock
revision: 1
---
unit: story:engineering-protocol-lock, worktree loom-w1121c-u3 at head 3e768c32 plus one untracked test file
verdict: green (no red case; 3 untested checks found, all `warning`)
cases: executed 59→70, red 0 (59 is the protocols count in impl-report; 70 is from `cargo test -p b10x-loom-protocols --locked` after my file existed)
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: 1 path: ~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w3/u3/adv1/ (12M: scripts, logs, and a probe crate with its own small build output)
needs-coordinator: none

1. `git --no-pager diff --stat` prints nothing, because my one file is untracked. The only path I added is a test file: `crates/loom-protocols/tests/adversary_w20261121w3_u3.rs`.

2. **My cases.** File: `~/.local/state/worktree/trees/b10x/loom/loom-w1121c-u3/crates/loom-protocols/tests/adversary_w20261121w3_u3.rs`. I ran it alone before any suite: exit 0, `test result: ok. 11 passed; 0 failed`. All 11 are green now. What they cover:
- 18 bad path forms (`..`, an absolute path, backslash, NUL, upper case, `%2D`, trailing `/`, `./`, depth 3, majors `01`/`0`/`+1`, `.yml`/`.YAML`, `lock.json`, a Cyrillic letter, the empty string). Each is refused Malformed at that path.
- Six bad `sha256` forms: a `sha256:` prefix, 63 or 65 digits, a non-hex digit, upper case, empty.
- A bad `commit`, an empty `tag`, and the wrong document shape (an array, `files` as an object, an entry that is not an object).
- The same lock in other bytes: a byte-order mark, CR LF, no final newline, two final newlines, `\u0030`, an entry member given twice, files out of order.
- Missing is refused before any definition is parsed.
- One path given twice in the file set: no catalog is built.
- Near-miss paths are refused Unlocked; a zero-byte locked file is a Definition error, not a panic.
- The embedded files and lock are the directory on disk now, so a stale embed would fail.

3. **Suite and checks, after my file existed.** All in `<adv1>/suite.sh`, log in `<adv1>/suite.out`:
- `cargo test -p b10x-loom-protocols --locked`: exit 0, 70 passed in total.
- `ESS_TOOLCHAIN=0.86.0 slot task intake-drift`: exit 0, "generated/rust/intake: no drift from ess/intake/".
- `ESS_TOOLCHAIN=0.86.0 slot task docs-check`: exit 0, and no `UNMAPPED:` marker.

4. **Findings.** Each check below is one the unit's suite never reaches; my test covers it now.
- `crates/loom-protocols/src/lock.rs:159`: deleting the 40-lowercase-hex `commit` check leaves the unit's suite green. No case feeds a wrong commit value, and "missing commit" is caught earlier by the member check. `a_commit_that_is_not_40_lowercase_hex_is_malformed` catches it. Warning, CONFIRMED, introduced.
- `lock.rs:136`: deleting the `!text.is_empty()` guard survives. An empty `tag` would then put an empty revision on every entry. `an_empty_tag_is_malformed` catches it. Warning, CONFIRMED, introduced.
- `lock.rs:280-287`: moving the Missing check after the definitions are added survives. The unit's only Missing case has nine valid files. `a_missing_file_is_refused_before_any_definition_is_parsed` catches it. Warning, CONFIRMED, introduced.
- `lib.rs:196`: what `engineering()` returns as its error text is never observed. Changing `error.to_string()` to `{error:?}` survives both suites. A refusal can only reach `engineering()` through a bad embedded file set, which a test cannot build. Note, INFEASIBLE, introduced.
- `build.rs:27,77,92`: no committed test reaches the refusal of a link, the refusal of a linked directory, or "every file at any depth". Mutants that drop any of these survive. My probe below shows the unmutated script works. Note, INFEASIBLE, introduced.
- `lock.rs:262`: the doc says "Every file is checked", but `check` returns at the first refusal. Note, CONFIRMED, introduced.

5. **Sound.**
- No admission path skips the digest check. `from_vendored` is the only constructor with a non-test caller (`engineering()`); a refused lock returns Err with no partial catalog; nothing is built lazily or held in a static.
- The decoder only accepts the encoder's exact bytes, so a duplicate member can never pass. I found no accepted input that two readers would read differently.
- Digests are computed over the raw embedded bytes, and every digest is compared before anything is parsed.
- I ran an identical copy of `build.rs` in a scratch crate (`<adv1>/probe.log`), never cleaning the build:
  - files added, changed and removed at depth 1 and 3 are picked up;
  - a missing `lock.json` embeds as 0 bytes;
  - a link to a file or a directory panics at `build.rs:92`;
  - `protocols/engineering` as a link panics at `:26`.
- The ten vendored files equal upstream (`git show 70e6eff4:protocols/<p>`) byte for byte, and each equals its lock line. Upstream has exactly ten `<major>.yaml` files.
- No governor, intake-router or executor file changed, and no manifest or `Cargo.lock` changed. `6d24610c` touches only `ess/`. There is no `/home/` literal, and nothing under `.engineering/` changed. The two CHANGELOG bullets are at the end of their groups.
- The governor still reads `canon_engineering::registry` at `crates/loom-governor/src/lib.rs:1738`. The two sources cannot disagree while `the_vendored_files_equal_the_pinned_registry` holds.

6. **Could not establish.**
- An empty lock with no files loads as an empty catalog rather than being refused. Reaching it needs an edit to the lock, which the brief already decided not to raise.
- There is no `.gitattributes`, so a checkout with `core.autocrlf=true` would refuse every file. That fails closed, and I found nothing that reaches it.
- I did not run `consumer-check` or `catalog_route.rs`.

`git status --short`: `?? crates/loom-protocols/tests/adversary_w20261121w3_u3.rs`

```findings
- {file: crates/loom-protocols/src/lock.rs, line: 159, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: no unit case feeds a wrong commit so deleting the 40-hex commit check survives}
- {file: crates/loom-protocols/src/lock.rs, line: 136, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: no unit case feeds an empty tag so deleting the non-empty guard survives}
- {file: crates/loom-protocols/src/lock.rs, line: 280, category: mutant, severity: warning, verdict: CONFIRMED, origin: introduced, message: Missing checked after admission survives because the only Missing case has nine valid files}
- {file: crates/loom-protocols/src/lib.rs, line: 196, category: mutant, severity: note, verdict: INFEASIBLE, origin: introduced, message: the text engineering() maps its refusal to is unobservable without a bad embed}
- {file: crates/loom-protocols/build.rs, line: 92, category: mutant, severity: note, verdict: INFEASIBLE, origin: introduced, message: no committed test reaches the link refusals or the any-depth listing of the build script}
- {file: crates/loom-protocols/src/lock.rs, line: 262, category: judgement, severity: note, verdict: CONFIRMED, origin: introduced, message: the check doc says every file is checked while it returns at the first refusal}
```
