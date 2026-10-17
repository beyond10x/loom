---
title: Configure model routes
sidebar_position: 5
description: Name the models Loom runs on with one llm.routes/1 document and a loom.yaml that gives each role a route.
lede: One routes document declares the endpoints and routes, a short loom.yaml gives the agent and the classifier a route each, and the command line and the SDK read both the same way.
source: crates/loom-sdk/src/models.rs, crates/loom-cli/src/lib.rs, crates/loom-docs/tests/model_routes.rs
---

# Configure model routes

Loom takes its models from an [llm](https://beyond10x.github.io/llm/) `llm.routes/1` document: it
declares endpoints (where, which wire, how to authenticate) and routes (a name for one
`<endpoint>/<model-id>`). llm's own guide,
[Configure model routes](https://beyond10x.github.io/llm/docs/guides/configure-model-routes),
holds the full format; this page shows the document Loom reads and how Loom names a model from it.

## The routes document

Save this as `routes.yaml`. It declares a gateway with a bearer token, the same gateway
reached anonymously, Anthropic directly and OpenRouter directly:

```yaml
format: llm.routes/1
endpoints:
  gateway:
    base_url: https://gateway.example.com/v1
    protocol: chat-completions
    auth: {kind: bearer, secret: "env:GATEWAY_TOKEN"}
  gateway-open:
    base_url: https://open.gateway.example.com/v1
    protocol: chat-completions
    auth: {kind: anonymous}
  anthropic:
    base_url: https://api.anthropic.com/v1
    protocol: messages
    auth: {kind: api-key, header: x-api-key, secret: "env:ANTHROPIC_API_KEY"}
  openrouter:
    base_url: https://openrouter.ai/api/v1
    protocol: chat-completions
    auth: {kind: bearer, secret: "env:OPENROUTER_API_KEY"}
routes:
  agent: anthropic/claude-sonnet-4-5
  classifier: openrouter/anthropic/claude-haiku-4.5
  shared: gateway/qwen
  open: gateway-open/qwen
```

A `secret` is a reference, never a value: `env:<VARIABLE>` or `file:<absolute path or ~/path>`.
Loom reads nothing itself; the variable or file is read when the first turn prepares its
credential, and one that is missing is refused as `unauthorized` before any request is sent.
Loom's build does not enable llm's `keychain:` source, so a `keychain:` reference is refused.

A route value splits on its first `/` only, so `openrouter/anthropic/claude-haiku-4.5` is endpoint
`openrouter`, model `anthropic/claude-haiku-4.5`. A name that is no route but reads
`<declared endpoint>/<model-id>` resolves directly.

## The configuration

Save this as `loom.yaml`, beside `routes.yaml`. `routes` is the path of the routes document,
relative to the directory of `loom.yaml`, and each role names a route of it:

```yaml
routes: routes.yaml
roles:
  - role: agent
    model: agent
  - role: classifier
    model: classifier
```

The roles are `agent`, the model that chooses each action, and `classifier`, the model that routes
the intent. A field other than `routes` and `roles`, a role other than these two, and a role listed
twice are refused, naming the file and the role.

## Run with it

```sh
b10x-loom run --config loom.yaml "fix the failing test" --workspace .
```

Each role runs on its route. `--model` or `--classifier-model`, when given, overrides its role
with another name of the same document, a route or an `<endpoint>/<model-id>` of a declared
endpoint:

```sh
b10x-loom run --config loom.yaml --model anthropic/claude-opus-4-1 "fix the failing test" --workspace .
```

A role that is neither configured nor given, and a model that cannot take a forced tool call, stop
the run before any model call. `--config` does not take `--catalog`.

With `--state <DIR>`, the commission's resume context keeps the configuration's absolute path and
the name that serves the role `agent` (`--model`, when given). A later `resume <COMMISSION> --state
<DIR>`, from any directory, loads that configuration again and continues on the same route, with
the credential its routes document names. A configuration that is gone, a routes document that no
longer serves the name, or a secret that does not resolve stops the resume, exit status 1, naming
the file and the role `agent`, before any model call. See [Run an intent](run-an-intent.md) for
the rest of the command.

## Load the models from the SDK

A host that embeds Loom reads the same files through `loom_sdk::models`:

```rust
use std::path::Path;
use loom_sdk::models::{self, ModelRole};

// Every role the configuration declares, over the routes document it names.
let models = models::from_config(Path::new("loom.yaml"))?;
let agent = models.get(ModelRole::Agent);
let another = models.model("anthropic/claude-opus-4-1")?;

// One name of a routes document, with no configuration file.
let model = models::from_routes(Path::new("routes.yaml"), "agent")?;
```

`loom_sdk::models::from_config` returns the model of each declared role; `loom_sdk::models::from_routes` returns the
model one name of a routes document serves. Both refuse with a message that names the file and
the role or name.

## Current limit

Every endpoint of the routes document must resolve, including one no role uses: llm builds the
secret resolvers for all endpoints at once, so a `file:~/` secret without `HOME`, or a `keychain:`
secret, refuses the whole document even when no role is served by that endpoint. Declare only
endpoints you can resolve. The limit is tracked at
[beyond10x/llm#36](https://github.com/beyond10x/llm/issues/36).
