---
format: aep.planning-md/3
id: review-result:engineering-protocols-as-vendored-data-acceptance-r2
kind: review-result
status: active
title: Plan critic acceptance, round 2, on epic:engineering-protocols-as-vendored-data
relations:
- reviews: epic:engineering-protocols-as-vendored-data
- reviews: story:engineering-protocol-lock
- reviews: story:engineering-protocols-vendor-command
- reviews: story:engineering-registry-calls-replaced
- reviews: story:engineering-protocols-dependency-removed
revision: 1
---
needs-revision

- `story:engineering-protocols-vendor-command` — the Outcome promises "removes a vendored file the tag no longer holds", but the cases only say "Each case runs the command with `--root` at a temporary root". The `t2` bullet is true if `t2` is run into a fresh root, so no listed case observes the removal. The body must say the `t2` case starts from the `t1` output and asserts `incident-response/1.yaml` is gone. — `.engineering/planning/story/engineering-protocols-vendor-command.md:23`, `:79`, `:89`
- `story:engineering-protocol-lock` — the epic says a malformed lock is refused "naming the file". The body gives `VendoredError::Lock` "the path", but defines no path for `Malformed` when the lock does not decode. `a_malformed_lock_is_refused_at_load` asserts only `Malformed`. The body must define the path `Malformed` carries (`lock.json`, or the offending member's path) and the test must assert it. — `.engineering/planning/story/engineering-protocol-lock.md:35`, `:144`; epic `.engineering/planning/epic/engineering-protocols-as-vendored-data.md:127`
- `story:engineering-registry-calls-replaced` — "`release-check` accepts it (`crates/loom-xtask/tests/release_check.rs` passes)" does not observe the new bullet. That test's repository case runs `release-check` at `0.57.2` (`release_check.rs:656-667`), and the bullet sits under `[Unreleased]`, which the case never reads. The acceptance must state a check that reads it, such as a fixture run or the bullet text by `git grep`. — `.engineering/planning/story/engineering-registry-calls-replaced.md:93-95`
- `story:engineering-registry-calls-replaced` — "The governor's doc comments name the vendored catalog, not the ELS registry (`:129-133`, `:1699`, `:1723`)" has no check. The lines say "the ELS registry", "built-in registry" and "`b10x-canon-engineering` is pinned to tag `0.15.2`" (`crates/loom-governor/src/lib.rs:51,130,132,1699,1723`). The bullet should say what each reads afterwards and give a grep, for example `git grep -n 'ELS registry' -- crates/loom-governor/src` printing nothing. — `.engineering/planning/story/engineering-registry-calls-replaced.md:91`
- `story:engineering-protocols-dependency-removed` — "`website/docs/concepts/governor-and-intake.md:15-16` say the same" has no content to check. Those lines name neither `b10x-canon-engineering` nor a tag, so the file reads the same before and after the work. The bullet also has no grep for the three pages. Name what governor-and-intake.md gains, or drop it from the bullet, and add `git grep -n 'b10x-canon-engineering' -- website` printing nothing. — `.engineering/planning/story/engineering-protocols-dependency-removed.md:96-98`

Round-1 fixes checked and holding:
- The load is now observed. `engineering_is_the_vendored_load` and the `git grep -n 'canon_engineering' -- crates/loom-protocols/src` bullet both fail on today's code (`lib.rs:160,162`).
- `repository` is the constant `lock::REPOSITORY`, and the story 2 test asserts it against a checkout whose `origin` differs.
- The epic's digest bullet names `the_engineering_digests_are_the_released_ones`, which story 3 keeps. I checked all ten literals against `git show 70e6eff:protocols/<p>.yaml | sha256sum` and they match.
- The epic names the vendor command and `protocols_vendor.rs`.
- Story 3's lock-failure mapping is a unit test over `builtin_in` with a tampered set, which fails today because `builtin_in` does not exist.
- The order after `story:canon-0-9-0-pin` is recorded: story 1 `depends_on` it, stories 2 and 3 depend on story 1, and story 4 depends on stories 2 and 3.
- The story 4 `AGENTS.md` grep list matches `39f4da88` (`:35`, `:520`, `:522`, `:523`, `:530`).

What I read: 5 artifacts (the epic and 4 stories) in full through `aep plan artifact show`, plus the round-1 verdict, `e5r1-changes.md`, the graph edges and the two critic files. In the tree I opened `crates/loom-governor/src/lib.rs`, `release_check.rs`, `adversary_w8_pin.rs`, the three website pages, `AGENTS.md` at `HEAD` and in the working tree, and the engineering-protocols `ls-tree` and digests.

What I could not establish:
- Whether the "12 sites assert what they asserted before" holds. It is a diff judgement with no mechanical check, and I treat it as acceptable.
- I did not run `cargo tree -i` on an absent package, so I cannot say whether it exits non-zero as story 4 assumes.

Out of my lane (did not set the verdict):
- Scope: `crates/loom-governor/src/lib.rs:129-133` says `b10x-canon-engineering` is pinned to tag `0.15.2`. Story 3 touches those lines for "ELS registry", but story 4 does not list the file, so no story owns the claim once the dependency goes.
- Parallel-safety: story 3 and story 4 both edit `CHANGELOG.md`. They are ordered by the story 4 edge, so I count this as settled.

```findings
- file: .engineering/planning/story/engineering-protocols-vendor-command.md
  line: 79
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises the command removes a vendored file the tag no longer holds, but the cases only run it at a temporary root, so the t2 bullet is true on a fresh root and no case starts from the t1 output and asserts incident-response/1.yaml is gone"
- file: .engineering/planning/story/engineering-protocol-lock.md
  line: 144
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the epic says a malformed lock is refused naming the file, but the body defines no path for Malformed and a_malformed_lock_is_refused_at_load asserts only Malformed, not a path"
- file: .engineering/planning/story/engineering-registry-calls-replaced.md
  line: 95
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the release-check accepts it clause is observed by release_check.rs, which runs release-check at the released 0.57.2 entry and never reads the new Unreleased bullet, so nothing checks that the Breaking bullet is accepted"
- file: .engineering/planning/story/engineering-registry-calls-replaced.md
  line: 91
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the governor doc comments name the vendored catalog, not the ELS registry has no check and does not say what lib.rs:129-133, :1699 and :1723 read afterwards, so it is true when someone says it is"
- file: .engineering/planning/story/engineering-protocols-dependency-removed.md
  line: 98
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "governor-and-intake.md:15-16 say the same has no content, since those lines name neither the crate nor a tag and read the same before and after, and the website bullet has no grep that shows the crate name gone from the three pages"
```
