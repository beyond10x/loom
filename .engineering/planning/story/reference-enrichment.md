---
format: aep.planning-md/3
id: story:reference-enrichment
kind: story
status: draft
title: Referenced data is fetched and put into the turn's context before the first model call
tags:
- idea
relations:
- serves: vision:O1
- informed_by: story:yaml-only-plugin
revision: 1
---
## Idea

Not scheduled. The operator, 2026-10-09: "follow up for it, automatic extraction, eg parse jira
ticket, fetch jira ticket, put compact version of it into context without having agent to think
about calling tools to get the data".

An `enrich` hook between classify/project and the turn:

1. run `b10x-loom-intake-references` (`crates/loom-intake-references/src/lib.rs`) on the item to
   find its references (tracker keys, chat permalinks, merge and pull requests, URLs);
2. fetch each reference through the Connectors read tools (for example a Jira issue get, Slack
   `conversations.replies` for a permalink);
3. put a compact, bounded summary of each into the turn's context before the first model call.

A failed fetch is noted in the context, and the model does not retry it. Fetched text is data, not
authority or verified evidence (`AGENTS.md` § Rules).

In the YAML-only plugin (`story:yaml-only-plugin`) it is declared per reference kind: which read
operation, which fields to keep, and the size bound.

## Open

- How the summary's bound relates to the 64 KiB serialized request ceiling and the bounded context
  policy (`AGENTS.md` § Intake and the command line).
- Whether a fetched record enters the briefing's result store or only the turn's context.
