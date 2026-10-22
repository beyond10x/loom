---
format: aep.planning-md/3
id: approval-record:disk-stop-and-go-2026-10-22-w1
kind: approval-record
status: draft
title: 'Disk stop and go during wave 2026-10-22-w1: one tree builds at a time'
relations:
- decides: story:host-stop-before-step
- decides: story:child-read-only-roots-and-host-paths
revision: 2
---
# Disk stop and go during wave 2026-10-22-w1

Session `b10x-chef-orchestrator` stopped the wave's builds, in these words: "Disk emergency: df / 14G
free at 03:5x (73G at 03:30). Stop your cargo runs now and start no new ones until I send a go at
30G. du: loom-w20261022w1-u2 target 11824 MB, u3 6191 MB, u1 5877 MB. Two cargo runs of
b10x-loom-intake-slice were live together. Run cargo clean in every unit tree that is not the one you
need next, and reply with the MB freed. After the go: one tree builds at a time."

- Done: the u2 test run and the u4 build were killed; `cargo clean` in the merged u1 and u3 trees
  freed 12068 MB.

It then released them: "Go: df / 46G at 04:0x. Build u2 first, then u4, one tree at a time through
b10x-build-slot. cargo clean each unit tree once it is merged. Record this go in your store."

- From here: u2 builds alone; u4 builds only after u2 reported; each unit tree is cleaned with
  `cargo clean` once merged; every cargo run sets `CARGO_INCREMENTAL=0
  CARGO_PROFILE_DEV_DEBUG=line-tables-only`.
