---
format: aep.planning-md/3
id: approval-record:agent-runtime-readme
kind: approval-record
status: draft
title: Loom described as the agent runtime
revision: 1
---
# Loom described as the agent runtime

Session `b10x-chef-orchestrator` relayed, approved by Timo (chat, 2026-10-22): "please describe
Loom now as the agent runtime. Put it one sentence in the README". It asked to change the opening
line of `README.md` so the first sentence names Loom as the agent runtime, keep the same wording on
the site intro and in `AGENTS.md`, and ship it as its own bot pull request.

- Done: the sentence "Loom is the agent runtime: it runs governed agents, and `b10x-loom-sdk` is
  its library for building on it." opens `README.md`, `AGENTS.md`, `website/docs/index.md` and the
  lede in `website/product.json`.
