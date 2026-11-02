---
format: aep.planning-md/3
id: story:release-publishes-images
kind: story
status: active
title: A release tag publishes the loom-agent and loom-operator images and the loom-agents chart to ghcr
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T10:20:24Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T10:20:24Z", actor: "human:timo", revision: 3}
---
## Outcome

Every release tag publishes two public `linux/amd64` container images and the Helm chart to the
GitHub container registry, each at the release version: `ghcr.io/beyond10x/loom-agent:<version>`,
which holds `b10x-loom` (the agent runtime every agent pod runs),
`ghcr.io/beyond10x/loom-operator:<version>`, which holds `b10x-loom-operator` (the controller), and
the chart `oci://ghcr.io/beyond10x/charts/loom-agents:<version>`. A downstream platform deploys
them as they are and adds only configuration.

## Why

The operator asked that Loom "provide Docker images which we can deploy from the GitHub registry";
a downstream platform adds only configuration on top (item D377, 2026-11-02). Until now the chart
named `registry.example/...` and every adopter built its own image. The image names follow the
names the chart already used (`loom-agent`, `loom-operator`; D383).

## Acceptance

- `deploy/images/loom-agent/Dockerfile` and `deploy/images/loom-operator/Dockerfile` build the
  release binary of `b10x-loom-cli` (`b10x-loom`) and of `b10x-loom-operator` with `--locked` on a
  digest-pinned Rust image, and copy it alone onto a digest-pinned minimal runtime base that runs
  as the numeric non-root user `10001`, the user the chart and the controller's pods run as. The
  build fails when the binary names a shared library the runtime base lacks, or when it does not
  answer `--version`.
- `.github/workflows/images.yml` builds both images. On a pull request that touches the
  Dockerfiles, the workflow, `Cargo.lock` or `Cargo.toml` it builds them without pushing; called
  from `release.yml` with `push: true` it refuses a version the registry already holds and pushes
  `<image>:<version>`. Every action is pinned by commit SHA.
- With `push: true` the same workflow packages `charts/loom-agents` and pushes it as the OCI
  artifact `oci://ghcr.io/beyond10x/charts/loom-agents:<version>`, refusing a version the registry
  already holds.
- `release.yml` calls `images.yml` only after `release / verify tag` and `release / check` pass;
  that one job is granted `packages: write`, every other job stays read-only.
- `crates/loom-xtask/tests/release_check.rs` holds the workflows' shape: the trigger, the one
  widened job, the pinned actions, the image names and the chart reference.

## Out of scope

- An `arm64` image (the operator asked for `linux/amd64`).
- Making a new package public: a package's visibility is a registry setting outside the repository.
