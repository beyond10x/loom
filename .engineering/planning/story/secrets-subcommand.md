---
format: aep.planning-md/3
id: story:secrets-subcommand
kind: story
status: draft
title: b10x-loom secrets re-uses the secrets CLI as a sub-command
relations:
- serves: vision:O1
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-cli/Cargo.toml
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/secrets.rs
- confidence: inferred
  path: ess/intake/domains/secrets.yaml
- confidence: inferred
  path: ess/intake/system.yaml
- confidence: inferred
  path: generated/rust/intake
- confidence: cited
  path: website/docs/reference/cli.md
revision: 10
---
## Outcome

`b10x-loom secrets …` manages the secrets Loom's runs use (model credentials, connector tokens)
by re-using beyond10x/secrets' `secretsctl` command surface as a sub-command: the same verbs
(`namespace add`, `put` from a hidden prompt or stdin, `describe`, `list`, `delete`, `rename`) on
the same native keychain layout, so a value stored with either tool resolves in both. A guided
`b10x-loom secrets setup claude-subscription` runs Claude Code's own `claude setup-token` flow and
stores the token it prints straight into the store, never showing it.

## Why

Operator, 2026-10-05: "can fully be automated by you ... file for later: \"loom secrets\" cli
command effectively re-using the secrets cli as a sub-command". The live subscription run
(llm docs/live-qualification.md) needed three manual steps: install secretsctl, `claude
setup-token`, `secretsctl put`.

## Open

- UNMAPPED: whether secretsctl exposes its clap command as a library (`secretsctl::Cli`) Loom can
  embed, or Loom shells out to an installed `secretsctl`; the former needs a secrets change.
- UNMAPPED: which secrets release first ships the `put`/`namespace` verbs (on secrets main after
  v0.5.0, not released at filing time).

## Not in scope

Changing the secrets library's storage layout; any login flow other than the vendor's own.
