---
format: aep.planning-md/3
id: review-result:plugin-layer-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, plugin-layer, round 2
relations:
- reviews: epic:plugin-layer
- reviews: story:inbound-answer-protocol
- reviews: story:plugin-host
- reviews: story:slack-plugin
- reviews: story:plugin-measurement
revision: 1
---
needs-revision

Origin key: `pre-existing` means round 1 raised the defect and it still holds. `introduced` means the text is new in revision 2. `undecided` means I could not tell. The store holds no pre-revision bodies (HEAD already carries revision 2), so I traced origin only through the round-1 findings.

- epic:plugin-layer — the two measurement lines name no test, though line 23 says each line is decided by the test the named story lists, and `story:plugin-measurement` lists none — .engineering/planning/epic/plugin-layer.md:33
- epic:plugin-layer — "refuses a pair outside the configured sources" is decided by `effects_refuse_an_undeclared_read`, which the story states against the projection, a different set — .engineering/planning/epic/plugin-layer.md:31
- story:inbound-answer-protocol — "declares artifact `item`, evidence kinds … outcome `declined`" is decided only by Canon compile, which passes without those names, and no test reaches `declined` — .engineering/planning/story/inbound-answer-protocol.md:36
- story:plugin-host — "A confidence below the threshold" names no threshold (value or config key), and the `SlackConfig` in `story:slack-plugin` has none, so `low_confidence_runs_no_turn` cannot be decided — .engineering/planning/story/plugin-host.md:62
- story:plugin-host — "The turn composes what `loom-intake-slice` composes … an authority provider that grants `source.read` and `reply.propose` only" states an implementation, and its one test checks only that the turn reaches `proposed`, so no check refuses a wider grant — .engineering/planning/story/plugin-host.md:70
- story:plugin-host — "nothing leaves the host" is a universal negative and `propose_writes_the_record_only` names no observation that decides it (for example an empty fake `connectors` argv log) — .engineering/planning/story/plugin-host.md:80
- story:plugin-host — the title promises a loop, but the only `plugin run` outcome decided is `--once`, so polling again, an interval or stopping has no outcome or test — .engineering/planning/story/plugin-host.md:88
- story:slack-plugin — scope lists "registering the plugin" in `crates/loom-cli/src/`, but no bullet or test decides that `b10x-loom plugin run slack-handler` resolves, which is the epic's one stated outcome — .engineering/planning/story/slack-plugin.md:82
- story:slack-plugin — "another seed may differ" is true whatever the code does, so it is a clause no check can fail; delete it or name a seed pair that differs on the fixtures — .engineering/planning/story/slack-plugin.md:55
- story:slack-plugin — "A message with a thread reply, or a reaction by the bot, is not an item" joins two independent conditions under one test name (as does "Bot and system messages" at line 64), so one can pass while the other fails — .engineering/planning/story/slack-plugin.md:62
- story:slack-plugin — "A config missing the Slack data source or the bot user id is refused" joins two conditions, and the one test is named for the bot id only, so the missing-data-source refusal has no test — .engineering/planning/story/slack-plugin.md:66
- story:plugin-measurement — the line-count bullet is decided by the page's own recorded command and output, and names no recount that must match; "per hook" has no stated counting rule — .engineering/planning/story/plugin-measurement.md:28
- story:plugin-measurement — the completeness check is the page's statement that two lists are equal, not a command that decides it, and the named `find` also matches `tests/*.rs` and omits `loom-connectors`' `cli` module, so equality with "one row per Rust module of those crates" cannot hold as written — .engineering/planning/story/plugin-measurement.md:31

**What I read:** 7 of 7 ids (epic:plugin-layer, the five stories, architecture-decision-record:plugin-hooks) with `aep plan artifact show` on each. I also read `aep plan artifact kinds`, all four round-1 review-results, the committed story bodies via `git show HEAD:`, and `git grep` for `fn bundled`/`fn plugins`. I ran no other `aep` read verbs or lifecycle queries.

**What I could not establish:**
- `ProtocolCatalog::plugins()` does not exist yet (only `bundled()` at `crates/loom-protocols/src/lib.rs:91`), which is expected for a draft story.
- Which plugin the `plugin run` CLI test in `story:plugin-host` uses before the Slack crate exists.
- Whether the `story:slack-plugin` fixture run goes through the CLI. This is the question behind the registration finding.
- The ADR has no acceptance section, which is normal for its kind, so I judged nothing there.
- Out of my lane, and they did not set the verdict:
  - The ADR says `inbound.answer/1` and "behind `ConnectorInvoker`", while the stories use `inbound-answer@1` and a plain CLI client (design).
  - Several stories list README, CHANGELOG, status.json and crates.md in scope with no acceptance line (scope).
  - `story:inbound-answer-protocol` was promoted to active while I read, and its body is unchanged.

Round 1 blockers and warnings now fixed:
- The "refused before any process starts" contradiction in `story:connectors-cli-reads`. The undeclared-kind and write-operation tests now say which processes ran.
- The "ask or find" ambiguity and the source-read contradiction in the `story:slack-plugin` fixture.
- The nonexistent `story:yaml-only-plugin` citation in `story:plugin-measurement`.
- The joined epic bullets. These are now split with test names, except the measurement lines above.

```findings
[
{"file":".engineering/planning/epic/plugin-layer.md","line":33,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the two measurement lines name no test, though line 23 says each line is decided by the test the named story lists, and story:plugin-measurement lists none"},
{"file":".engineering/planning/epic/plugin-layer.md","line":31,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"\"refuses a pair outside the configured sources\" is decided by effects_refuse_an_undeclared_read, which the story states against the projection, a different set"},
{"file":".engineering/planning/story/inbound-answer-protocol.md","line":36,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"pre-existing","message":"\"declares artifact item, evidence kinds … outcome declined\" is decided only by Canon compile, which passes without those names, and no test reaches declined"},
{"file":".engineering/planning/story/plugin-host.md","line":62,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"undecided","message":"\"A confidence below the threshold\" names no threshold (value or config key), and the SlackConfig in story:slack-plugin has none, so low_confidence_runs_no_turn cannot be decided"},
{"file":".engineering/planning/story/plugin-host.md","line":70,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"undecided","message":"\"The turn composes what loom-intake-slice composes … an authority provider that grants source.read and reply.propose only\" states an implementation, and its one test checks only that the turn reaches proposed, so no check refuses a wider grant"},
{"file":".engineering/planning/story/plugin-host.md","line":80,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"undecided","message":"\"nothing leaves the host\" is a universal negative and propose_writes_the_record_only names no observation that decides it (for example an empty fake connectors argv log)"},
{"file":".engineering/planning/story/plugin-host.md","line":88,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"undecided","message":"the title promises a loop, but the only plugin run outcome decided is --once, so polling again, an interval or stopping has no outcome or test"},
{"file":".engineering/planning/story/slack-plugin.md","line":82,"category":"acceptance","severity":"blocker","verdict":"needs-revision","origin":"undecided","message":"scope lists \"registering the plugin\" in crates/loom-cli/src/, but no bullet or test decides that b10x-loom plugin run slack-handler resolves, which is the epic's one stated outcome"},
{"file":".engineering/planning/story/slack-plugin.md","line":55,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"\"another seed may differ\" is true whatever the code does, so it is a clause no check can fail; delete it or name a seed pair that differs on the fixtures"},
{"file":".engineering/planning/story/slack-plugin.md","line":62,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"pre-existing","message":"\"A message with a thread reply, or a reaction by the bot, is not an item\" joins two independent conditions under one test name (as does \"Bot and system messages\" at line 64), so one can pass while the other fails"},
{"file":".engineering/planning/story/slack-plugin.md","line":66,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"pre-existing","message":"\"A config missing the Slack data source or the bot user id is refused\" joins two conditions, and the one test is named for the bot id only, so the missing-data-source refusal has no test"},
{"file":".engineering/planning/story/plugin-measurement.md","line":28,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"pre-existing","message":"the line-count bullet is decided by the page's own recorded command and output, and names no recount that must match; \"per hook\" has no stated counting rule"},
{"file":".engineering/planning/story/plugin-measurement.md","line":31,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"pre-existing","message":"the completeness check is the page's statement that two lists are equal, not a command that decides it, and the named find also matches tests/*.rs and omits loom-connectors' cli module, so equality with \"one row per Rust module of those crates\" cannot hold as written"}
]
```
