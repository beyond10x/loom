---
format: aep.planning-md/3
id: review-result:bound-program-result-table-w7-acceptance-3
kind: review-result
status: active
title: Plan critic (acceptance), wave 7 round 3, on story:bound-program-result-table
relations:
- reviews: story:bound-program-result-table
revision: 1
---
needs-revision

story:bound-program-result-table — the line "A request that carries no arguments … gives the program the literal" does not say what form such a request takes, so the test cannot be written from the story: the request's `arguments` is a `ProposedActionArguments` that is always present, never optional (`generated/rust/commission/src/responsibility.rs:3105`); a test cannot build an `AdmittedRequest` itself (`AdmittedRequest::new` is `pub(crate)`, `crates/loom-commission/src/ports/effect.rs:113`); and neither the Outcome nor `ess/domains/bindings.yaml` says whether JSON `null`, `{}` or both mean "no arguments" — .engineering/planning/story/bound-program-result-table.md:177
story:bound-program-result-table — the Outcome sentence that the admission's `NoRunner` names neither `--unconfined-bindings` nor `unconfined_bindings: true` has no Acceptance line, though a test could check it on the refusal's `detail`; the no-runner line (:225) checks only the variant — .engineering/planning/story/bound-program-result-table.md:58-60
story:bound-program-result-table — the non-UTF-8 `PATH` answer is left to review because "a test cannot set a non-UTF-8 `PATH` without changing the environment of its own process", but the tree already has a test that runs itself again as a child process with a `PATH` the parent sets (`crates/loom-intake-slice/tests/adversary_w20261021w6_u3.rs:73-83`), so a test can check that this step is refused, starts no program and records nothing — .engineering/planning/story/bound-program-result-table.md:305-306

Round-2 findings: all five are closed.
- The `git grep -n unconfined` check: closed. It is replaced by the decoder's member lists and the four ESS structs (:53-58), and I checked the cites: `decode.rs:14-41`, `bindings.yaml:37,89,162,189`.
- The `cargo test` command: closed. It now runs the five files, and `git grep -l -e unconfined_bindings -e ActionBindings -- crates/loom-intake-slice/tests` prints exactly those five at `aa943153` (:277-290).
- The writer line: closed (:227-231). The test parses the shared helper's protocol, and `model::Action::effect: Option<EffectClass>` exists in canon 0.9.0 (`crates/canon/src/model/mod.rs:203`).
- The recording-runner contradiction: closed (:132).
- Outcome clauses without a line: closed by the lines at :174, :177 and :195.

The other new lines hold:
- `upstream_revisions`: the port can write it, because `EvidenceData.facts` is carried unchanged through `submit_evidence` (`evidence.rs:116-133`) in the `canon-evidence/1` shape that `pass2.rs:166-172` writes. A test can read it back through `CanonGovernor::evidence(&case)` (`loom-governor/src/lib.rs:1123`). It is red before the story.
- The request-wins line (:174) is observable through the `cat` blob.
- The Outcome's `select_runner` order (flag's reason, then file opt-in, then admission), the file opt-in winning over a runner that is not `local`, and the `PATH` error and refused step all match `bindings.rs:366-400` and `:491-494`.

What I read: 1 artifact at store revision 17 (it matches `w7-bprt-body-3.md`), `diff -u` of the two drafts, and at `aa943153` `bindings.rs`, `decode.rs`, `bindings.yaml`, `effect.rs`, `evidence.rs`, `responsibility.rs`, `loom-governor/src/lib.rs`, `pass2.rs:37-62` and `:150-190`, `adversary_w20261021w6_u3.rs:70-85`, and canon `model/mod.rs`.

What I could not establish: none.

```findings
- file: .engineering/planning/story/bound-program-result-table.md
  line: 177
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the line 'A request that carries no arguments … gives the program the literal' does not say what form such a request takes, so the test cannot be written from the story: the request's arguments is a ProposedActionArguments that is always present, a test cannot build an AdmittedRequest itself (AdmittedRequest::new is pub(crate)), and neither the Outcome nor ess/domains/bindings.yaml says whether JSON null, {} or both mean no arguments"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 58
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome sentence that the admission's NoRunner names neither --unconfined-bindings nor unconfined_bindings: true has no Acceptance line, though a test could check it on the refusal's detail; the no-runner line checks only the variant"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 305
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the non-UTF-8 PATH answer is left to review because a test supposedly cannot set a non-UTF-8 PATH without changing its own environment, but adversary_w20261021w6_u3.rs:73-83 already runs itself again as a child process with a PATH the parent sets, so a test can check that this step is refused, starts no program and records nothing"
```
