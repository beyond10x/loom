//! Harness's integration suites for the two ported wires, carried from `beyond10x/harness` at
//! `798325f03cf5a18df8fadb346d31b314826136ec` case for case with only their paths rewritten:
//! `tests/contract.rs` and `tests/summary_request.rs` of `harness-messages` and
//! `harness-responses`, and `tests/transport.rs` of `harness-messages`. They hold the ported
//! adapters to every fixture under `tests/fixtures/provider-wires/` (the OAuth request, the
//! streams, the manifests and the event inventories), which are read at run time through
//! `CARGO_MANIFEST_DIR`. Not carried: `tests/provider_emulated.rs` in both crates, which drives a
//! Python endpoint over a socket.

// Harness `crates/harness-messages/tests/contract.rs`.
mod messages_contract {
    //! The pinned wire subset, replayed through the code a live turn uses.
    //!
    //! A contract that is only prose drifts silently. These two fixtures are the wire: change what the
    //! harness sends or what it accepts, and one of them stops matching.

    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use b10x_loom_executor::harness::messages::{
        ACCEPTED_CONTENT_BLOCK_DELTAS, ACCEPTED_STREAM_EVENTS, SUBSCRIPTION_CLIENT_PREAMBLE, WIRE,
        contract_headers, decode_stream, request_body,
    };
    use b10x_loom_executor::harness::wire::{
        Approval, CallId, CredentialKind, Envelope, Item, Sampling, StopReason, ToolCall,
        ToolChoice, ToolName, ToolOutcome, ToolSpec, TurnRequest, Usage, VecSink, WireId,
    };
    use serde_json::{Value, json};

    /// The cut that added the subscription client preamble. `2026-08-30` is the same wire without it,
    /// `2026-08-29b` the one before `tool_choice`, and `2026-08-29` the one before the rolling cache
    /// breakpoint; all three stay pinned as they were released.
    const VERSION: &str = "2026-08-31";

    fn contract_dir() -> PathBuf {
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo test"))
            .join("..")
            .join("..")
            .join("crates")
            .join("loom-executor")
            .join("tests")
            .join("fixtures")
            .join("provider-wires")
            .join(WIRE)
            .join(VERSION)
    }

    fn fixture(name: &str) -> String {
        let path = contract_dir().join("fixtures").join(name);
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("reading `{}`: {error}", path.display()))
    }

    fn fixture_bytes(name: &str) -> Vec<u8> {
        fs::read(contract_dir().join("fixtures").join(name)).expect("readable fixture")
    }

    fn manifest() -> Value {
        serde_json::from_str(
            &fs::read_to_string(contract_dir().join("manifest.json")).expect("readable"),
        )
        .expect("the manifest is JSON")
    }

    fn strings(value: &Value) -> Vec<String> {
        value
            .as_array()
            .expect("an array")
            .iter()
            .map(|entry| entry.as_str().expect("a string").to_owned())
            .collect()
    }

    /// The canonical turn: an instruction, a person's input, a replayed thinking block, one call and
    /// its result. Every field the harness ever sends appears here.
    fn canonical_request() -> Value {
        canonical_request_as(None)
    }

    /// The same turn, projected under a named credential presentation.
    ///
    /// **The presentation is an argument because on this route it changes the body.** A subscription
    /// token is served only when `system` opens with the client preamble as its own block, so the
    /// contract pins two request fixtures rather than one and this is the single place either is
    /// built — a second builder would prove only that the second builder works.
    fn canonical_request_as(credential: Option<CredentialKind>) -> Value {
        let items = vec![
            Item::user("read the readme"),
            Item::Opaque {
                wire: WireId::new(WIRE).expect("valid"),
                payload: json!({
                    "type": "thinking",
                    "thinking": "OPAQUE-REASONING-BLOB",
                    "signature": "OPAQUE-SIGNATURE",
                }),
            },
            Item::assistant("Reading the readme."),
            Item::ToolCall(ToolCall {
                call_id: CallId::new("toolu_1").expect("valid"),
                name: ToolName::new("workspace_read").expect("valid"),
                arguments: json!({"path": "README.md"}),
            }),
            Item::result(
                CallId::new("toolu_1").expect("valid"),
                ToolOutcome::ok(json!({"text": "hello harness"})),
            ),
        ];
        let tools = vec![ToolSpec {
            name: ToolName::new("workspace_read").expect("valid"),
            description: "Read one text file inside the workspace.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": false,
            }),
            approval: Approval::NotRequired,
            envelope: Envelope::default(),
        }];
        request_body(
            &TurnRequest {
                model: "b10x-emulated".to_owned(),
                instructions: "be useful".to_owned(),
                items,
                tools,
                // Resolved by the caller on this route, and passed beside the turn below.
                max_output_tokens: None,
                // Set, not defaulted, for the same reason the first wire's fixture sets them.
                sampling: Sampling {
                    temperature: Some(0.2),
                    top_p: Some(0.95),
                    reasoning_effort: Some("medium".to_owned()),
                },
                // Held to the turn's own tool, because the fixture's job is to carry **every** field
                // the harness sends and this one is only sent when a caller holds a turn. `auto` is
                // what the other turns of a run send, and what they send is nothing.
                tool_choice: ToolChoice::Named(ToolName::new("workspace_read").expect("valid")),
            },
            // Named, not defaulted: this route requires an output bound, so the fixture pins what one
            // looks like rather than pinning that it can be left out — it cannot.
            4096,
            credential,
        )
    }

    #[test]
    fn the_request_the_harness_sends_matches_the_pinned_fixture() {
        let expected = fixture_bytes("turn-request.json");
        let actual = b10x_loom_executor::harness::http::encode_json_body(&canonical_request())
            .expect("encodes");
        assert_eq!(
            actual, expected,
            "the exact request bytes changed; cut a contract and say so in the changelog"
        );
    }

    #[test]
    fn the_request_a_subscription_token_sends_matches_its_own_pinned_fixture() {
        let expected = fixture_bytes("turn-request-oauth.json");
        let actual = b10x_loom_executor::harness::http::encode_json_body(&canonical_request_as(
            Some(CredentialKind::Oauth),
        ))
        .expect("encodes");
        assert_eq!(
            actual, expected,
            "the exact subscription request bytes changed; cut a contract"
        );
    }

    #[test]
    fn every_header_name_and_non_secret_value_is_pinned_for_each_presentation() {
        let rows = |kind| {
            Value::Array(
                contract_headers(kind)
                    .into_iter()
                    .map(|(name, value)| json!({"name":name, "value":value}))
                    .collect(),
            )
        };
        let pinned = &manifest()["request_headers"];
        assert_eq!(rows(None), pinned["none"]);
        assert_eq!(rows(Some(CredentialKind::ApiKey)), pinned["api-key"]);
        assert_eq!(rows(Some(CredentialKind::Oauth)), pinned["oauth"]);
    }

    #[test]
    fn the_manifest_pins_both_production_event_inventories_and_terminal_policy() {
        let accepted: Value =
            serde_json::from_str(&fixture("accepted-events.json")).expect("inventory JSON");
        assert_eq!(accepted["stream_events"], json!(ACCEPTED_STREAM_EVENTS));
        assert_eq!(
            accepted["content_block_deltas"],
            json!(ACCEPTED_CONTENT_BLOCK_DELTAS)
        );
        assert_eq!(manifest()["stream_events"], accepted["stream_events"]);
        assert_eq!(
            manifest()["content_block_deltas"],
            accepted["content_block_deltas"]
        );
        assert_eq!(manifest()["terminal_sentinel"]["required"], json!(false));
        assert_eq!(
            manifest()["terminal_sentinel"]["terminal_event"],
            "message_stop"
        );
    }

    /// The whole of what this version cut a directory for.
    ///
    /// Asserted field by field rather than only by whole-body equality above, because a reader of the
    /// contract is entitled to find the rule stated: **block 0 is the preamble, exactly, alone**, and
    /// the run's own instruction is the block after it. Every other shape measured on 2026-08-30
    /// answered `429` with no rate-limit headers, which reads downstream as an exhausted quota.
    #[test]
    fn a_subscription_token_opens_the_system_with_the_client_preamble_and_nothing_else() {
        let oauth = canonical_request_as(Some(CredentialKind::Oauth));
        let system = oauth["system"].as_array().expect("an array");

        assert_eq!(system.len(), 2, "{oauth}");
        assert_eq!(system[0]["text"], json!(SUBSCRIPTION_CLIENT_PREAMBLE));
        assert_eq!(system[0]["type"], json!("text"));
        // Alone: the preamble block carries the string and no breakpoint, so nothing can be appended
        // to it later without this failing.
        assert_eq!(
            system[0].as_object().expect("an object").len(),
            2,
            "the preamble block carries text and type and nothing else: {oauth}"
        );
        assert_eq!(system[1]["text"], json!("be useful"));

        // The breakpoint moves to the last block so the constant head stays cached — see the
        // projection's own note. Under a key issued to a program there is one block and it is both.
        assert_eq!(
            system[1]["cache_control"],
            json!({"type": "ephemeral"}),
            "{oauth}"
        );
        assert!(system[0].get("cache_control").is_none(), "{oauth}");

        let key = canonical_request();
        assert_eq!(key["system"].as_array().expect("an array").len(), 1);
        assert_eq!(key["system"][0]["text"], json!("be useful"));
    }

    #[test]
    fn the_pinned_request_carries_both_cache_breakpoints_and_nothing_opaque_is_marked() {
        // The whole of what `2026-08-29b` cut a version for. Asserted against the **fixture** rather
        // than against the projection, because the projection is what the test above already compares:
        // this one says what a reader of the contract is entitled to find in it.
        let request: Value = serde_json::from_str(&fixture("turn-request.json"))
            .expect("the request fixture is JSON");

        // The constant head: `tools` then `system` is everything before the conversation.
        assert_eq!(
            request["system"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );

        // The rolling one: the last content block of the last message, which is where the conversation
        // grows. Without it the growth is re-charged in full on every remaining turn.
        let messages = request["messages"].as_array().expect("an array");
        let last = messages.last().expect("a last message");
        assert_eq!(last["role"], json!("user"));
        let blocks = last["content"].as_array().expect("an array");
        assert_eq!(
            blocks.last().expect("a last block")["cache_control"],
            json!({"type": "ephemeral"})
        );

        // Two, against a cap of four. A third would have to be argued for.
        assert_eq!(
            request.to_string().matches("cache_control").count(),
            2,
            "{request}"
        );

        // And never on a replayed thinking block: its signature covers the block as the model produced
        // it, so an added key is a rejected turn (AGENTS.md invariant 5).
        let thinking = messages
            .iter()
            .flat_map(|message| message["content"].as_array().expect("an array"))
            .find(|block| block["type"] == json!("thinking"))
            .expect("the canonical turn replays one");
        assert_eq!(
            *thinking,
            json!({
                "type": "thinking",
                "thinking": "OPAQUE-REASONING-BLOB",
                "signature": "OPAQUE-SIGNATURE",
            })
        );
    }

    #[test]
    fn the_manifest_names_exactly_the_request_fields_the_harness_sends() {
        let sent: Vec<String> = canonical_request()
            .as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(sent, strings(&manifest()["request_fields"]));
    }

    #[test]
    fn the_pinned_stream_decodes_into_the_expected_turn() {
        let mut sink = VecSink::new();
        let outcome = decode_stream(
            "b10x-emulated",
            fixture("turn-stream.sse").as_bytes(),
            &mut sink,
        )
        .expect("the pinned stream decodes");

        assert_eq!(sink.text(), "Reading the readme.");
        assert_eq!(outcome.stop_reason, StopReason::ToolCalls);
        // 31 fresh, 7 read from cache, 5 written to it. The neutral total is the sum, because
        // `Usage::input_tokens` is the whole and the cache figures are parts of it — this route
        // reports them disjointly and the projection is what reconciles the two.
        assert_eq!(
            outcome.usage,
            Some(Usage {
                model: "b10x-emulated".to_owned(),
                input_tokens: 43,
                output_tokens: 11,
                cached_input_tokens: 7,
                cache_creation_input_tokens: Some(5),
            })
        );

        let kinds: Vec<&str> = outcome
            .items
            .iter()
            .map(|item| match item {
                Item::Opaque { .. } => "opaque",
                Item::ToolCall(_) => "tool-call",
                Item::AssistantText { .. } => "assistant-text",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, vec!["opaque", "assistant-text", "tool-call"]);

        // The thinking block is carried whole, signature and all, and never reinterpreted.
        assert_eq!(
            outcome.items[0],
            Item::Opaque {
                wire: WireId::new(WIRE).expect("valid"),
                payload: json!({
                    "type": "thinking",
                    "thinking": "Checking.",
                    "signature": "SIG",
                }),
            }
        );

        // And shown while it arrived. The pinned stream carries one `thinking_delta` and one
        // `signature_delta`; only the first is reasoning a person should see, and the signature is
        // never shown. An event count here is what catches the summary being emitted twice.
        let reasoning: Vec<&str> = sink
            .events()
            .iter()
            .filter_map(|event| match event {
                b10x_loom_executor::harness::wire::StreamEvent::ReasoningDelta { text } => {
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(reasoning, vec!["Checking."]);

        let call = outcome
            .tool_calls()
            .next()
            .expect("the pinned stream carries one call");
        assert_eq!(call.call_id.as_str(), "toolu_1");
        assert_eq!(call.arguments, json!({"path": "README.md"}));

        // Nothing in the pinned stream may be unrecognized: a warning here means the subset drifted.
        assert!(
            sink.events().iter().all(|event| !matches!(
                event,
                b10x_loom_executor::harness::wire::StreamEvent::Warning { .. }
            )),
            "{:?}",
            sink.events()
        );
    }

    #[test]
    fn the_manifest_names_exactly_the_stream_events_the_fixture_carries() {
        let seen: BTreeSet<String> = fixture("turn-stream.sse")
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|payload| {
                serde_json::from_str::<Value>(payload).expect("each event is JSON")["type"]
                    .as_str()
                    .expect("each event has a type")
                    .to_owned()
            })
            .collect();
        let pinned: BTreeSet<String> = strings(&manifest()["stream_events"]).into_iter().collect();
        assert!(seen.is_subset(&pinned));
    }

    #[test]
    fn the_manifest_names_exactly_the_content_block_deltas_the_fixture_carries() {
        // A second layer the first wire does not have: on this route the interesting variation is
        // inside `content_block_delta`, so pinning the outer event names alone would pin almost
        // nothing.
        let seen: BTreeSet<String> = fixture("turn-stream.sse")
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|payload| {
                let event: Value = serde_json::from_str(payload).expect("each event is JSON");
                event
                    .get("delta")
                    .and_then(|delta| delta.get("type"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect();
        let pinned: BTreeSet<String> = strings(&manifest()["content_block_deltas"])
            .into_iter()
            .collect();
        assert_eq!(seen, pinned);
    }

    #[test]
    fn the_pinned_error_event_takes_the_typed_retryable_path() {
        let mut sink = VecSink::new();
        let error = decode_stream(
            "b10x-emulated",
            fixture("error-stream.sse").as_bytes(),
            &mut sink,
        )
        .expect_err("the error is terminal");
        assert!(error.retriable, "{error:?}");
    }
}

// Harness `crates/harness-responses/tests/contract.rs`.
mod responses_contract {
    //! The pinned wire subset, replayed through the code a live turn uses.
    //!
    //! A contract that is only prose drifts silently. These two fixtures are the wire: change what the
    //! harness sends or what it accepts, and one of them stops matching.

    use std::fs;
    use std::path::PathBuf;

    use b10x_loom_executor::harness::responses::{
        ACCEPTED_STREAM_EVENTS, WIRE, contract_headers, decode_stream, request_body,
    };
    use b10x_loom_executor::harness::wire::{
        Approval, CallId, Envelope, Item, Sampling, StopReason, ToolCall, ToolChoice, ToolName,
        ToolOutcome, ToolSpec, TurnRequest, Usage, VecSink, WireId,
    };
    use serde_json::{Value, json};

    /// The cut that modeled the route's keepalive marker as progress. `2026-08-31` is the same wire
    /// without it and stays pinned as released.
    const VERSION: &str = "2026-08-31.1";

    fn contract_dir() -> PathBuf {
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo test"))
            .join("..")
            .join("..")
            .join("crates")
            .join("loom-executor")
            .join("tests")
            .join("fixtures")
            .join("provider-wires")
            .join(WIRE)
            .join(VERSION)
    }

    fn fixture(name: &str) -> String {
        let path = contract_dir().join("fixtures").join(name);
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("reading `{}`: {error}", path.display()))
    }

    fn fixture_bytes(name: &str) -> Vec<u8> {
        fs::read(contract_dir().join("fixtures").join(name)).expect("readable fixture")
    }

    fn manifest() -> Value {
        serde_json::from_str(
            &fs::read_to_string(contract_dir().join("manifest.json")).expect("readable"),
        )
        .expect("the manifest is JSON")
    }

    /// The canonical turn: an instruction, a person's input, a replayed reasoning item, one call and
    /// its result. Every field the harness ever sends appears here.
    fn canonical_request() -> Value {
        let items = vec![
            Item::user("read the readme"),
            Item::Opaque {
                wire: WireId::new(WIRE).expect("valid"),
                payload: json!({
                    "id": "rs_1",
                    "type": "reasoning",
                    "summary": [],
                    "encrypted_content": "OPAQUE",
                }),
            },
            Item::ToolCall(ToolCall {
                call_id: CallId::new("call_1").expect("valid"),
                name: ToolName::new("workspace_read").expect("valid"),
                arguments: json!({"path": "README.md"}),
            }),
            Item::result(
                CallId::new("call_1").expect("valid"),
                ToolOutcome::ok(json!({"text": "hello harness"})),
            ),
        ];
        let tools = vec![ToolSpec {
            name: ToolName::new("workspace_read").expect("valid"),
            description: "Read one text file inside the workspace.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": false,
            }),
            approval: Approval::NotRequired,
            envelope: Envelope::default(),
        }];
        request_body(
            // Fixed here so the pinned fixture stays byte-stable; a real run's is per-conversation.
            "b10x-session-fixture",
            &TurnRequest {
                model: "b10x-emulated".to_owned(),
                instructions: "be useful".to_owned(),
                items,
                tools,
                max_output_tokens: Some(4096),
                // Set, not defaulted. A fixture with the sampling fields absent would pin only that
                // they can be left out, which is what the previous version already pinned.
                sampling: Sampling {
                    temperature: Some(0.2),
                    top_p: Some(0.95),
                    reasoning_effort: Some("medium".to_owned()),
                },
                // Held to the turn's own tool, because the fixture's job is to carry **every** field
                // the harness sends and this one is only sent when a caller holds a turn. What every
                // other turn of a run sends for it is nothing.
                tool_choice: ToolChoice::Named(ToolName::new("workspace_read").expect("valid")),
            },
        )
    }

    #[test]
    fn the_request_the_harness_sends_matches_the_pinned_fixture() {
        let expected = fixture_bytes("turn-request.json");
        let actual = b10x_loom_executor::harness::http::encode_json_body(&canonical_request())
            .expect("encodes");
        assert_eq!(
            actual, expected,
            "the exact request bytes changed; cut a contract and say so in the changelog"
        );
    }

    #[test]
    fn every_non_secret_request_header_and_value_is_pinned() {
        let mut actual = contract_headers("b10x-session-fixture", 7)
            .into_iter()
            .map(|(name, value)| json!({"name": name, "value": value}))
            .collect::<Vec<_>>();
        actual.push(json!({"name":"authorization", "value":"<credential omitted>"}));
        assert_eq!(Value::Array(actual), manifest()["request_headers"]);
    }

    #[test]
    fn the_manifest_pins_the_production_event_inventory_and_terminal_sentinel() {
        let accepted: Value =
            serde_json::from_str(&fixture("accepted-events.json")).expect("inventory JSON");
        assert_eq!(accepted["stream_events"], json!(ACCEPTED_STREAM_EVENTS));
        assert_eq!(manifest()["stream_events"], accepted["stream_events"]);
        assert_eq!(manifest()["terminal_sentinel"]["required"], json!(true));
        assert_eq!(manifest()["terminal_sentinel"]["bytes"], "data: [DONE]\n\n");
    }

    #[test]
    fn the_manifest_names_exactly_the_request_fields_the_harness_sends() {
        let sent: Vec<String> = canonical_request()
            .as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect();
        let pinned: Vec<String> = manifest()["request_fields"]
            .as_array()
            .expect("an array")
            .iter()
            .map(|value| value.as_str().expect("a string").to_owned())
            .collect();
        assert_eq!(sent, pinned);
    }

    #[test]
    fn the_pinned_stream_decodes_into_the_expected_turn() {
        let mut sink = VecSink::new();
        let outcome = decode_stream(
            "b10x-emulated",
            fixture("turn-stream.sse").as_bytes(),
            &mut sink,
        )
        .expect("the pinned stream decodes");

        assert_eq!(sink.text(), "Reading the readme.");
        assert_eq!(outcome.stop_reason, StopReason::ToolCalls);
        assert_eq!(
            outcome.usage,
            Some(Usage {
                model: "b10x-emulated".to_owned(),
                input_tokens: 42,
                output_tokens: 11,
                cached_input_tokens: 7,
                cache_creation_input_tokens: None,
            })
        );

        let kinds: Vec<&str> = outcome
            .items
            .iter()
            .map(|item| match item {
                Item::Opaque { .. } => "opaque",
                Item::ToolCall(_) => "tool-call",
                Item::AssistantText { .. } => "assistant-text",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, vec!["opaque", "tool-call", "assistant-text"]);

        let call = outcome
            .tool_calls()
            .next()
            .expect("the pinned stream carries one call");
        assert_eq!(call.call_id.as_str(), "call_1");
        assert_eq!(call.arguments, json!({"path": "README.md"}));

        // Nothing in the pinned stream may be unrecognized: a warning here means the subset drifted.
        assert!(
            sink.events().iter().all(|event| !matches!(
                event,
                b10x_loom_executor::harness::wire::StreamEvent::Warning { .. }
            )),
            "{:?}",
            sink.events()
        );
    }

    #[test]
    fn the_manifest_names_exactly_the_stream_events_the_fixture_carries() {
        let seen: std::collections::BTreeSet<String> = fixture("turn-stream.sse")
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter(|payload| *payload != "[DONE]")
            .map(|payload| {
                serde_json::from_str::<Value>(payload).expect("each event is JSON")["type"]
                    .as_str()
                    .expect("each event has a type")
                    .to_owned()
            })
            .collect();
        let pinned: std::collections::BTreeSet<String> = manifest()["stream_events"]
            .as_array()
            .expect("an array")
            .iter()
            .map(|value| value.as_str().expect("a string").to_owned())
            .collect();
        assert!(seen.is_subset(&pinned));
    }

    #[test]
    fn the_pinned_incomplete_and_failure_events_take_their_typed_paths() {
        let mut sink = VecSink::new();
        let incomplete = decode_stream(
            "b10x-emulated",
            fixture("incomplete-stream.sse").as_bytes(),
            &mut sink,
        )
        .expect("incomplete is a terminal turn");
        assert_eq!(incomplete.stop_reason, StopReason::MaxOutputTokens);

        for fixture_name in ["failed-stream.sse", "error-stream.sse"] {
            let mut sink = VecSink::new();
            let failure =
                decode_stream("b10x-emulated", fixture(fixture_name).as_bytes(), &mut sink)
                    .expect_err("the pinned failure is typed");
            assert!(failure.retriable, "{fixture_name}: {failure:?}");
        }
    }
}

// Harness `crates/harness-messages/tests/summary_request.rs`.
mod messages_summary_request {
    //! What the loop's summariser sends, projected through this wire.
    //!
    //! This is the route the defect was found on. Sent as items, the fold began with an assistant-side
    //! message — and this route requires the first message to be `user` — and carried `tool_use` and
    //! `tool_result` blocks while the summary request publishes `tools: []`, which it also rejects. So
    //! every compaction on this wire paid for a turn that could not be answered.
    //!
    //! [`b10x_loom_executor::harness::turn_loop::summary_request_items`] renders the fold to text instead, and this projects its
    //! output to prove the shape is wire-neutral rather than merely tolerated here.

    use b10x_loom_executor::harness::messages::{WIRE, request_body};
    use b10x_loom_executor::harness::wire::{
        CallId, Item, Sampling, ToolCall, ToolChoice, ToolName, ToolOutcome, TurnRequest, WireId,
    };
    use serde_json::json;

    /// The part of a conversation a compaction would fold: assistant-first, with a whole tool round
    /// trip and the reasoning item that preceded it.
    fn folded() -> Vec<Item> {
        let call_id = CallId::new("call-1").expect("a valid call id");
        vec![
            Item::assistant("I will read the file first."),
            Item::Opaque {
                wire: WireId::new(WIRE).expect("a valid wire id"),
                payload: json!({
                    "type": "thinking",
                    "thinking": "opaque",
                    "signature": "opaque",
                }),
            },
            Item::ToolCall(ToolCall {
                call_id: call_id.clone(),
                name: ToolName::new("file_read").expect("a valid tool name"),
                arguments: json!({"path": "README.md"}),
            }),
            Item::result(call_id, ToolOutcome::ok(json!({"text": "hello"}))),
            Item::user("and now summarise it"),
        ]
    }

    fn body(items: &[Item]) -> serde_json::Value {
        request_body(
            &TurnRequest {
                model: "test-model".to_owned(),
                instructions: "the standing instruction".to_owned(),
                items: items.to_vec(),
                tools: Vec::new(),
                max_output_tokens: None,
                sampling: Sampling::default(),
                tool_choice: ToolChoice::Auto,
            },
            1024,
            // This suite is about how a fold reaches the wire, not about the credential; the shape
            // under a subscription token differs only by a leading `system` block the contract pins.
            None,
        )
    }

    #[test]
    fn a_summary_request_projects_to_one_user_message_with_no_tool_blocks() {
        let projected =
            body(&b10x_loom_executor::harness::turn_loop::summary_request_items(&folded()));
        let messages = projected["messages"]
            .as_array()
            .expect("this route carries its conversation in `messages`");

        assert_eq!(
            messages.len(),
            1,
            "one message, so the first one is `user` whatever the fold held: {messages:?}"
        );
        assert_eq!(messages[0]["role"], "user", "{:?}", messages[0]);
        for block in messages[0]["content"]
            .as_array()
            .expect("a message carries a block list")
        {
            assert_eq!(
                block["type"], "text",
                "a request that publishes no tools may carry no tool blocks, and a thinking block \
                 belongs only to the conversation that produced it: {block}"
            );
        }
        assert_eq!(
            projected["tools"],
            json!([]),
            "a summary turn has nothing to call"
        );
    }

    #[test]
    fn the_fold_sent_as_items_is_the_request_this_replaces() {
        // Kept as evidence rather than as prose: both refusals are visible in one projection, and a
        // change that put the raw fold back would make this pass again.
        let projected = body(&folded());
        let messages = projected["messages"]
            .as_array()
            .expect("this route carries its conversation in `messages`");

        assert_eq!(
            messages[0]["role"], "assistant",
            "the fold begins after the task, so its first message is assistant-side"
        );
        let rendered = projected.to_string();
        assert!(
            rendered.contains("tool_use") && rendered.contains("tool_result"),
            "and it carries tool blocks while `tools` is empty"
        );
    }
}

// Harness `crates/harness-responses/tests/summary_request.rs`.
mod responses_summary_request {
    //! What the loop's summariser sends, projected through this wire.
    //!
    //! The summary turn is a request like any other, and it used to be one this route would refuse: the
    //! fold it carried began with an assistant-side item, carried `function_call` and
    //! `function_call_output` entries while publishing no tools, and replayed opaque reasoning items
    //! into a request that was not the conversation they came from.
    //!
    //! [`b10x_loom_executor::harness::turn_loop::summary_request_items`] renders the fold to text instead, and this projects its
    //! output to prove the shape is wire-neutral rather than merely tolerated here.

    use b10x_loom_executor::harness::responses::{WIRE, request_body};
    use b10x_loom_executor::harness::wire::{
        CallId, Item, Sampling, ToolCall, ToolChoice, ToolName, ToolOutcome, TurnRequest, WireId,
    };
    use serde_json::json;

    /// The part of a conversation a compaction would fold: assistant-first, with a whole tool round
    /// trip and the reasoning item that preceded it.
    fn folded() -> Vec<Item> {
        let call_id = CallId::new("call-1").expect("a valid call id");
        vec![
            Item::assistant("I will read the file first."),
            Item::Opaque {
                wire: WireId::new(WIRE).expect("a valid wire id"),
                payload: json!({
                    "type": "reasoning",
                    "id": "rs_1",
                    "encrypted_content": "opaque",
                }),
            },
            Item::ToolCall(ToolCall {
                call_id: call_id.clone(),
                name: ToolName::new("file_read").expect("a valid tool name"),
                arguments: json!({"path": "README.md"}),
            }),
            Item::result(call_id, ToolOutcome::ok(json!({"text": "hello"}))),
            Item::user("and now summarise it"),
        ]
    }

    fn body(items: &[Item]) -> serde_json::Value {
        request_body(
            "b10x-session",
            &TurnRequest {
                model: "test-model".to_owned(),
                instructions: "the standing instruction".to_owned(),
                items: items.to_vec(),
                tools: Vec::new(),
                max_output_tokens: None,
                sampling: Sampling::default(),
                tool_choice: ToolChoice::Auto,
            },
        )
    }

    #[test]
    fn a_summary_request_projects_to_one_user_message_and_no_tool_or_reasoning_entries() {
        let projected =
            body(&b10x_loom_executor::harness::turn_loop::summary_request_items(&folded()));
        let input = projected["input"]
            .as_array()
            .expect("this route carries its conversation in `input`");

        assert_eq!(
            input.len(),
            2,
            "the standing instruction and one user message, and nothing else: {input:?}"
        );
        assert_eq!(input[0]["role"], "developer", "{:?}", input[0]);
        assert_eq!(input[1]["type"], "message", "{:?}", input[1]);
        assert_eq!(input[1]["role"], "user", "{:?}", input[1]);
        for entry in input {
            let kind = entry["type"].as_str().unwrap_or_default();
            assert!(
                !matches!(kind, "function_call" | "function_call_output" | "reasoning"),
                "a turn that publishes no tools may carry no tool entries, and a reasoning item \
                 belongs only to the conversation that produced it: {entry}"
            );
        }
        assert_eq!(
            projected["tools"],
            json!([]),
            "a summary turn has nothing to call"
        );
    }

    #[test]
    fn the_fold_sent_as_items_is_the_request_this_replaces() {
        // Kept as evidence rather than as prose: the defect was real on this route too, and a change
        // that put the raw fold back would make this pass again.
        let projected = body(&folded());
        let input = projected["input"]
            .as_array()
            .expect("this route carries its conversation in `input`");

        assert_eq!(
            input[1]["role"], "assistant",
            "the fold begins after the task, so its first item is assistant-side"
        );
        assert!(
            input
                .iter()
                .any(|entry| entry["type"] == "function_call" || entry["type"] == "reasoning"),
            "and it carries tool and reasoning entries while `tools` is empty"
        );
    }
}

// Harness `crates/harness-messages/tests/transport.rs`.
mod wires_transport {
    //! What the two wires ask of `harness-http`, compared.
    //!
    //! The transport half is one crate now, and the reason it could become one is that both wires
    //! wanted the same thing from it. That claim needs somewhere it can fail: a wire that quietly
    //! doubled its attempts or halved a timeout would otherwise be found by a person reading two files
    //! side by side, which is how the duplication survived a release in the first place.
    //!
    //! It lives in this crate's suite because both wires are visible here — `harness-responses` is a
    //! **dev**-dependency and nothing under `src/` may import it. Same shape as
    //! `provider_emulated.rs`'s `the_two_wires_serve_the_same_scenarios`, which compares the two
    //! emulators for the same reason.

    use b10x_loom_executor::harness::http::{Framing, RetryPolicy, Settings};
    use b10x_loom_executor::harness::messages::TRANSPORT as MESSAGES;
    use b10x_loom_executor::harness::responses::TRANSPORT as RESPONSES;

    #[test]
    fn the_two_wires_configure_one_transport_and_differ_only_in_their_framing() {
        // Field by field, by comparing whole values with the one permitted difference substituted in:
        // a setting added later is compared without anybody remembering to add a line here.
        assert_eq!(
            Settings {
                framing: MESSAGES.framing,
                ..RESPONSES
            },
            MESSAGES,
            "the two wires disagree about something other than framing; either that is a finding worth \
             a line in STATUS.md or one of them drifted"
        );
    }

    #[test]
    fn the_framing_is_the_difference_and_each_wire_names_its_own() {
        // The one thing that is genuinely per-route. The first route ends its stream with
        // `data: [DONE]`; the second has no sentinel at all and ends on a `message_stop` payload, so a
        // `[DONE]` line there is a payload that is not JSON and refuses as one. Unifying these would
        // have taught the second route a sentinel it does not speak.
        assert_eq!(RESPONSES.framing, Framing::DoneSentinel);
        assert_eq!(MESSAGES.framing, Framing::PayloadsOnly);
        assert_ne!(RESPONSES.framing, MESSAGES.framing);
    }

    #[test]
    fn both_wires_retry_on_the_shared_policy_rather_than_one_of_their_own() {
        // The numbers the emulated suites depend on: four attempts is what
        // `a_cold_gateway_is_retriable_transport_rather_than_a_refusal` reads back out of the message
        // both wires produce.
        assert_eq!(RESPONSES.retry, RetryPolicy::DEFAULT);
        assert_eq!(MESSAGES.retry, RetryPolicy::DEFAULT);
        assert_eq!(RetryPolicy::DEFAULT.max_attempts, 4);
    }
}
