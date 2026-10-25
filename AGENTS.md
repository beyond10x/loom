# AGENTS.md — loom

Loom is the agent runtime: it runs governed agents, and `b10x-loom-sdk` is its library for building
on it. This file is for an agent changing the repository. How to install Loom and how to embed it
are in [README.md](README.md) and on the site, <https://beyond10x.github.io/loom/>; neither is
repeated here. The architecture behind it is in Atlas (private): ADRs 0066–0076, 0080, 0082, 0089
and 0090 under `architecture/adr/`, and `docs/design/governed-autonomy/`. ADR 0090 made Loom the
one runtime repository, with Commission, the governor and intake moved in.

## Serves

- **O1**, governed reach.
- **O3**, any harness, observed and compared.

§ Boundary says what each asks of Loom.

## Boundary

Loom serves two outcomes of the governed-autonomy vision. **O1, governed reach:** the model sees
and can execute only what the current frontier admits, and authority is rechecked before every
effect. **O3, any harness, observed and compared:** Loom is the native executor Metaharness compares
against other harnesses on the same case.

Loom owns the decision path (ADR 0071, as ADR 0098 narrows it): prompt and context assembly, the
projected action catalogue, action selection, argument generation, the budgets of its model
requests, and model-facing approval suspension. Each decision is a structured request to an llm
`Model`; Loom runs no model loop of its own. Since ADR 0090 it also holds Commission's contracts
and runtime, the governor and intake (sections below).

It does not own protocol semantics (Canon), engineering protocol definitions
(`b10x-canon-engineering`, beyond10x/engineering-protocols), case truth, organizational authority,
connector credentials, final completion, provider wires or credentials (llm), or the engineering
record of a case (AEP).

Loom reaches models through llm only, for its decisions, and agent work in a harness (Harness,
Claude Code, Codex) through metaharness behind an `AgentExecutor` (ADR 0098). Loom deleted its
port of Harness (`story:delete-harness-port`); do not port a harness loop, wire or session into it
again.

## Protocol composition and system queries

Loom owns `protocols/system-query/1.yaml`, a read-only local clock query. Canon still owns protocol
semantics, and engineering-protocols still owns engineering definitions. `b10x-loom-protocols`
composes bundled and explicitly installed definitions. Intake routing, artifact declarations and
host admission must use the same immutable catalog; a remote package never installs executable
code or authority. Protocol installation is explicit and runs read verified snapshots offline.

`run --workspace` is optional for system queries and required for software changes. Route before
protocol-specific setup: a clock query must not touch Git, a workspace or Substrate. Software
changes retain confinement and merge approval. Clock evidence comes from one trusted reading,
not model output; only Canon completion makes a query successful. The integration test
`crates/loom-intake-slice/tests/system_query.rs::clock_query_runs_in_both_policies_without_workspace_resources`
holds this path in both context modes. `protocols add/list/remove` manage user installations;
`--replace` is explicit and bundled definitions cannot be shadowed. The SDK exports the catalog.
A registration has one form, `name@major`: lowercase letters, digits and hyphens before `@`,
starting with a lowercase letter and at most 100 bytes, then a positive canonical major; a
protocol id is not one. `self.improvement/1` registers as `self-improvement@1`, its dots and
underscores as hyphens and `/<major>` as `@<major>`, as the bundled `software-change@1` holds
`software.change/1`. A refusal of any other form names the registration its protocol id takes:
`ProtocolCatalog::add_yaml` and `protocols add --file` (`InstallStore::install_file`, which reads
the file to answer) derive it from the id and revision the definition declares when it parses,
else from the name given; `protocols remove` and `protocols add --source`, which have no
definition to read, from the name given. An id the mapping gives no registration for is refused
with the rule alone, and a refusal quotes at most 120 bytes of the refused name
(`crates/loom-protocols/tests/catalog.rs`,
`a_protocol_id_with_a_dot_registers_under_the_documented_name`;
`crates/loom-protocols/tests/adversary_w20261020w4_u1.rs`).

Loom also owns `protocols/inbound-answer/1.yaml` (`inbound-answer@1`), the read-only protocol a
plugin turn answers an inbound item under: `source.read` reads (capability `datasource.read`),
`reply.propose` records a proposal and `reply.decline` a decline, and neither sends anything. It
is the one exception to a single catalog: a plugin host admits on `ProtocolCatalog::plugins()`,
which holds it, while routing, `protocols list` and a plugin's task path read `bundled()`, which
does not. A plugin protocol is never routed to, and a routed protocol is never run by a plugin
turn. `crates/loom-protocols/tests/inbound_answer.rs` holds both catalogs.

## Rules

- Model-visible actions derive from the current frontier and runtime capability. Nothing
  consequential is granted because it was registered at startup (ADR 0072).
- A selector picks only from the candidate set it was given. Unknown action ids are rejected,
  confidence never grants authority, and low confidence falls back to a stronger path (ADR 0073).
- Revalidate every selected action against case revision, frontier and authority before
  execution. A proposal carries no case revision of its own; the runtime reads the current one.
- A trace is not evidence (ADR 0074). Evidence comes from a trusted verifier, never from what a
  model says happened.
- The model is not trusted context: it never supplies identity, authority, case revision, trusted
  time, approval results or tenant context.
- When a selector, integration, verifier or authority provider fails, fail toward less authority,
  less effect and more explicit uncertainty. Never silently broaden capability.
- Do not make Loom semantically dependent on one selector vendor or model.
- Commission core never depends on Loom's executor, on Canon or on a model-provider crate
  (ADR 0075). Loom depends on Commission and implements its `AgentExecutor`.
- Anything that runs is Rust; command lines use clap derive.

Commission's code and tests cite this section (`AGENTS.md` § Rules). Keep the heading and the
phrases above; `adversary_agents_rules_carry_the_rules_commission_cites` fails without them.

## Checks that hold the rules

| Claim | Held by |
|---|---|
| The catalogue follows the frontier | `crates/loom-executor/tests/frontier_projection.rs` (`projection_follows_frontier`) |
| A selector cannot leave the catalogue | `crates/loom-executor/tests/action_selector.rs` (`selector_cannot_leave_catalogue`) |
| The reasoning-model selector, `model_selector::ModelSelector` over an llm `Model`, offers the model only its candidates, and an answer outside them is never a selection | `crates/loom-executor/tests/reasoning_model_selector.rs` (`an_action_outside_the_candidate_set_is_a_selection_error_and_never_a_selection`) |
| Low confidence falls back to the stronger selector, at a host-supplied threshold compared as a number | `crates/loom-executor/tests/confidence_fallback.rs` (`the_fast_choice_is_returned_only_at_or_above_the_supplied_threshold`) |
| A confidence fallback is recorded as two selections, each with its own selector's strategy: the fast one `Overruled`, naming its replacement, which alone reaches argument generation and revalidation; a replacement that is unknown, the selection itself, no longer `Selected` or from another catalogue is refused and nothing is recorded; `Loom::select` returns the pick under its own selector's strategy | `crates/loom-executor/tests/fallback_selection_recording.rs` (`an_overruled_fast_selection_is_recorded_overruled_naming_its_replacement`, `overrule_selection_naming_an_unknown_replacement_is_refused_with_selection_not_found`); `crates/loom-executor/tests/adversary_w4_fallback_overrule.rs`; `crates/loom-executor/tests/adversary_w4_fallback_pipeline.rs` (`adversary_w4_loom_select_carries_the_strategy_of_the_selector_that_made_the_pick`) |
| Every selection gets one `SelectionRecord` equal to it, with `fell_back_to` on the overruled fast selection naming the replacement's strategy; a selection refused at the execution boundary adds no record; a record is never evidence | `crates/loom-executor/tests/selection_telemetry.rs` (`selection_telemetry_is_recorded`) |
| Laya selects behind the confidence fallback; arguments are generated once, only for the finally selected action, and an action outside the frontier never reaches argument generation or revalidation | `crates/loom-selector-laya/tests/laya_arguments_slice.rs` (`a_choice_outside_the_frontier_is_rejected_and_never_reaches_arguments_or_revalidation`) |
| A blocked or merge-seeking pick is never proposed | `crates/loom-executor/tests/adversary_executor_admission.rs` |
| A workflow asks a model only in a judgement step, offering it only that step's actions; a judgement step runs on the host's own `AgentExecutor` when it supplies one, called for that step only, told its instructions and handed only its actions, and a proposal outside them, or reads proposed together, never reach Commission; a step whose action the frontier does not list ends `NeedsHumanJudgment` naming it and nothing is proposed; a step advances only once its request was `Performed`; a definition naming an action outside its protocol is refused at load, naming the step | `crates/loom-executor/tests/workflow_executor.rs` (`deterministic_steps_call_no_model`, `a_judgement_step_offers_the_model_only_its_actions`, `a_judgement_step_runs_on_a_supplied_executor`, `a_supplied_executors_reads_proposed_together_are_never_proposed`, `a_step_the_frontier_does_not_list_ends_needing_human_judgment`, `a_step_advances_only_after_its_effect_was_performed`, `a_definition_naming_an_action_outside_its_protocol_is_refused`), `crates/loom-executor/tests/adversary_w11_u2.rs` |
| A later step's arguments carry an earlier step's typed output with no model call, and a missing or mistyped output, or one its step did not produce, proposes nothing and ends `NeedsHumanJudgment` naming the step and the output; a reference to a later, unknown or undeclared output is refused at load | `crates/loom-executor/tests/workflow_step_outputs.rs` (`a_later_step_reads_an_earlier_output_without_a_model`, `a_mistyped_output_ends_needing_human_judgment`, `an_absent_output_ends_needing_human_judgment`, `a_reference_to_a_later_or_unknown_step_is_refused_at_load`, `outputs_are_identical_on_a_rerun_with_the_same_reports`) |
| A judgement step sees exactly the earlier outputs it lists, typed, in one data block of its instructions that no value can close, never as instructions; a listed output the run did not produce asks the model nothing and ends `NeedsHumanJudgment` naming the step and the output; an input naming no earlier declared output, or listed twice, is refused at load | `crates/loom-executor/tests/workflow_step_outputs.rs` (`a_judgement_step_reads_a_listed_output_as_data`, `a_judgement_step_listing_an_absent_output_asks_no_model`, `a_judgement_step_input_naming_no_earlier_output_is_refused_at_load`) |
| A workflow run's record, written after every performed step to the governor's `FileWorkflowRunStore` keyed by the Run id, lets `resume(run_id)` in another process continue the run after an approval or a crash between steps, every output bound, no performed step invoked again, and the usage report covering every segment, what a host's executor reported for a judgement call included, counted once; a record of another Run is refused, never read | `crates/loom-commission-testkit/tests/workflow_run_store.rs` (`a_step_result_survives_a_restart_across_an_approval`, `the_usage_report_covers_every_segment_of_a_resumed_run`, `a_step_result_of_another_run_is_refused`, `a_workflow_resumes_after_a_crash_between_steps`), `crates/loom-executor/tests/workflow_executor.rs` (`the_run_record_carries_a_host_executors_usage_once`), `crates/loom-executor/tests/adversary_w20261021w5_u1.rs` |
| A workflow run whose judgement step's chooser reached its input-token budget or could not observe it is `Suspended` with `LoopBudget` carrying the stop as `loom.run.LoopStop` declares it, never `NoUsefulAction` | `crates/loom-executor/tests/workflow_executor.rs` (`a_judgement_step_stopped_by_its_input_token_budget_suspends_the_run_for_it`, `a_judgement_step_whose_budget_cannot_be_observed_suspends_the_run_for_it`) |
| A stale or unlisted request is refused at revalidation | `crates/loom-commission-testkit/tests/action_request.rs`, `crates/loom-executor/tests/adversary_run_revalidation.rs`, `crates/loom-executor/tests/selection_revalidation.rs` |
| A selection id the run already holds, in any state, is never selected again (`SelectAction` `selection-exists`), so a refused selection stays `Refused` and is never admitted; the run pipeline writes nothing and generates no arguments for a reused id, and `Loom::select` writes no record | `crates/loom-executor/tests/adversary2_run_identity.rs` (`a_refused_selection_is_never_selected_or_admitted_again`), `crates/loom-executor/src/lib.rs` (`the_pipeline_refuses_a_selection_id_its_record_already_holds`), `crates/loom-executor/tests/adversary_w20261024w2_u3.rs`, `task conform` |
| Unknown capabilities and panicking authority providers yield no grant | `crates/loom-commission-testkit/tests/adversary_authority_fail_closed.rs` |
| An authority provider is asked about the action request the runtime revalidated, at the run loop and on a resume, bound to the frontier's revision and never to one the arguments name; a grant bound to one revision allows no other; the runtime reads the case's revision again after the allow and once more right before a resumed request is invoked, and performs nothing on a move it sees: a resume that sees it before its take takes nothing, one that sees it after releases the Suspension; a move while the effect itself runs is the effect port's to refuse, by the request's `expected_case_revision`; a failing provider grants nothing at either check | `crates/loom-commission-testkit/tests/authority_request.rs` (`the_provider_sees_the_request_it_decides`, `a_grant_bound_to_one_revision_does_not_allow_the_next`, `a_failing_provider_still_grants_nothing`, `a_resume_that_sees_the_case_moved_after_its_take_releases_the_suspension_and_performs_nothing`), `crates/loom-commission-testkit/tests/adversary_w20261020_u2.rs`, `crates/loom-commission/tests/security_w20261020_u2.rs` |
| Commission names no executor, Canon, `model-provider-deny.txt`, Connectors or Mandate crate | `task commission:deps-guard` (`crates/loom-commission-testkit/tests/executor_port.rs`) |
| A Mandate authority provider grants only on Mandate's allow: a Mandate outage, an authority context that is no Mandate context and the empty case id grant nothing, every outcome answers as `loom.authority` declares, and each case id is its own Mandate resource, `case-<n>` included, so a grant on one case answers for no other | `crates/loom-mandate/tests/mandate_authority.rs` (`an_unavailable_mandate_grants_nothing`, `an_authority_context_that_is_no_mandate_context_grants_nothing`, `every_outcome_answers_as_the_specification_declares`, `a_governor_case_id_is_a_mandate_resource`, `distinct_case_ids_are_distinct_resources`), `crates/loom-mandate/tests/adversary_w10_u2.rs`, `crates/loom-mandate/tests/security_w10_u2.rs` |
| With `--mandate`, a resume performs the held request only when Mandate's `check_with_approval` clears and consumes the approval the grant names, kept in the state directory's log before anything is performed: a second resume with it, an expired approval, one naming another subject, action or resource, one recorded for another commission however its id is written, and a grant naming none are refused and perform nothing; without `--mandate`, a grant naming an approval is refused and performs nothing; the approver is neither the verified subject nor its actor; only `b10x-loom-cli` links `b10x-loom-mandate`, and only it and `loom-docs` reach Mandate over `Cargo.lock` | `crates/loom-cli/tests/resume_after_grant.rs` (`a_recorded_approval_clears_the_held_request_once`, `a_second_resume_with_the_same_approval_is_refused_and_performs_nothing`, `an_expired_approval_is_refused_and_performs_nothing`, `the_grant_names_its_mandate_approval`), `crates/loom-cli/tests/adversary_w10_u3.rs` (`adversary_a_resume_without_mandate_does_not_perform_a_grant_naming_a_mandate_approval`), `crates/loom-cli/tests/security_w10_u3.rs` (`security_an_approval_is_bound_to_its_commission_however_its_id_is_written`), `crates/loom-mandate/tests/mandate_approval.rs` (`an_approval_naming_another_subject_action_or_resource_is_refused`), `crates/loom-mandate/tests/adversary_w10_u3.rs` (`adversary_a_host_file_whose_approver_is_the_acting_actor_is_refused`), `crates/loom-mandate/tests/security_w10_u2.rs` |
| A case the governor holds completes only when exactly one declared outcome is legitimate, unless the host built it with `CompletionRule::FirstDeclared`, which takes the first legitimate one in the admitted definition's declaration order, never the decision document's map order; neither takes an outcome Canon holds blocked | `crates/loom-governor/tests/completion_rule.rs` (`first_declared_completes_on_the_first_declared_legitimate_outcome`, `a_blocked_outcome_is_never_completed_under_first_declared`, `with_one_legitimate_outcome_both_rules_agree`) |
| An executor's report that the case moved is decided by the governor and judged on the case's current frontier, reloaded once | `crates/loom-commission-testkit/tests/moved_case_outcome.rs`, `crates/loom-executor/tests/adversary_w1_runtime_stale.rs`, `crates/loom-executor/tests/adversary_w1p2_runtime_windows.rs` |
| An executor is never handed an unperformed action that needs no authority; an effect is invoked once, through its binding, and again only after the port answered `Pending` | `crates/loom-commission-testkit/tests/effect_invocation.rs` (`effect_invoked_only_through_its_binding`) |
| An effect port's `Pending` answer gets its request invoked again at the next iteration, under its own id and at its own revision, with no executor call between, revalidated before each invocation as before the first (authority asked again, the revision read again after an allow), until the port answers otherwise: a request whose case moved, that the frontier no longer admits or the provider denies is not invoked again, and one the provider answers `ApprovalRequired` for ends the run `NeedsAuthority`, holding a Suspension under `run_with_suspension`; a pending read of a fan-out is invoked again alone, and only while the fan-out rule still admits it; each invocation is a step and one trial, each answer one observation and never evidence; a pending answer neither joins nor restarts the idle count; a budget that ends the run while a request is pending ends it `Waiting`, naming the requests and the budget's reason, the Run suspended for it; the slice stops it `StepBudget` | `crates/loom-commission-testkit/tests/pending_effect.rs` (`a_pending_answer_is_invoked_again_and_not_counted_idle`, `a_pending_answer_leaves_the_idle_count_where_it_was`, `each_reinvocation_is_revalidated`, `a_budget_reached_while_pending_ends_the_run_waiting`, `each_answer_is_charged_and_observed`, `approval_required_when_a_pending_request_is_asked_again_ends_the_run_needing_authority`), `crates/loom-commission-testkit/tests/adversary_w20261020w3_u2.rs`, `crates/loom-intake-slice/src/run.rs` (`a_run_waiting_on_a_pending_request_stops_step_budget`), `task commission:conform` (`JudgeBudgetStop`) |
| A `Pending` answer marked `ends_run` is not invoked again: once recorded, charged one trial and observed once, it ends the run `Waiting` for `Time` with the port's report, naming every request the step left pending (a fan-out's in proposal order), after a proposal, an invocation again, a fan-out or a resume (its Suspension released); a spending limit passed beside it ends the run first, and a breach kept under `KeepStep` is the reason given; `ends_run: false` is invoked again as before | `crates/loom-commission-testkit/tests/pending_effect.rs` (`a_pending_answer_that_ends_the_run_is_not_invoked_again`, `a_pending_answer_that_ends_the_run_is_recorded_charged_and_observed_once`, `a_pending_answer_with_ends_run_false_is_invoked_again`, `a_pending_request_invoked_again_that_ends_the_run_is_not_invoked_a_third_time`, `a_fan_out_whose_read_ends_the_run_waits_on_every_pending_read`, `a_breach_beside_a_pending_answer_that_ends_the_run_ends_it_first`, `a_resumed_request_whose_pending_answer_ends_the_run_is_released_and_waits`), `crates/loom-commission-testkit/tests/run_and_action_budgets.rs` (`keep_step_ends_a_run_waiting_on_an_ending_pending_answer_for_its_breach`) |
| An effect refusal marked final (`ends_run`) ends the run at once with `EffectRefused`, naming the request and the reason, once it was recorded and observed: after a proposal, after a fan-out once every read was recorded (beside a stale read it is kept until the move is judged, and ends the run when the next load finds no move), and after a resumed request, whose Suspension is released; an ordinary refusal still counts idle | `crates/loom-commission-testkit/tests/refused_effect_outcome.rs` (`a_final_refusal_ends_the_run_naming_request_and_reason`, `an_ordinary_refusal_still_counts_idle`, `the_refusal_is_observed_once_and_recorded`), `crates/loom-commission-testkit/tests/adversary_w20261020w2_u1.rs`, `task commission:conform` (`JudgeEffectRefusal`) |
| Without a step budget, two idle iterations in a row end the run `IdleBound`, naming them, whatever kind of idle iteration they are, a repeated admission included, and a new admission starts the count again; a case the last one's performed effect completed ends the run completed instead, the case loaded first as at the step budget; a frontier that admits nothing still ends `NoAdmissibleAction`; the slice stops `IdleBound` `NothingAdmissible`, as before | `crates/loom-commission-testkit/tests/idle_bound_outcome.rs` (`two_idle_iterations_end_the_run_as_the_idle_bound`, `a_frontier_that_admits_nothing_ends_as_no_admissible_action`, `a_repeated_admission_counts_toward_the_idle_bound`, `a_case_its_repeated_reads_completed_ends_the_run_completed`), `crates/loom-commission-testkit/tests/adversary_w20261020w2_u2.rs`, `crates/loom-intake-slice/src/run.rs` (`an_idle_bound_run_stops_nothing_admissible_as_before`), `task commission:conform` (`JudgeIdleIteration`) |
| A host's stop policy (`LoopContext::stop_policy`) with the gate off never ends a run `AwaitingApproval`: an unchanged frontier goes to the executor and an idle step counts toward the idle bound, while a proposal needing approval is still asked about and one not allowed is never invoked (`NeedsAuthority`, or a held Suspension); its idle bound applies with or without a step budget, below 1 read as 1; a context that does not implement it keeps today's stops | `crates/loom-commission-testkit/tests/stop_policy.rs` (`an_unchanged_frontier_without_the_gate_is_an_idle_iteration`, `an_ungranted_approval_required_action_is_still_refused_without_the_gate`, `the_idle_bound_applies_with_a_step_budget_when_set`, `an_idle_bound_below_one_is_read_as_one`, `the_default_stop_policy_keeps_todays_stops`), `task commission:conform` (`JudgeIdleIteration`) |
| A run the effect port performs nothing for ends `NoPerformableAction` naming the actions it does not perform, each once, in frontier order, and the frontier's open claims (its claims not `True`, which the frontier relates to no action): before the executor runs, after an admitted proposal (that action alone) and after a move, one the governor bears out or one the runtime finds after a stale proposal; the slice's `NoLocalExecutor` stop carries the actions, and its `run --output jsonl` terminal record names them as `unperformed_actions` | `crates/loom-commission-testkit/tests/unperformable_names.rs` (`the_outcome_names_each_unperformed_action_and_its_claim`, `a_proposed_unperformed_action_is_named`, `an_action_listed_twice_is_named_once`, `a_moved_case_whose_frontier_performs_nothing_names_its_actions`), `crates/loom-commission-testkit/tests/adversary_w20261020w3_u1.rs` (`adversary_a_found_move_onto_nothing_performable_names_the_actions`), `crates/loom-cli/tests/run_events.rs` (`a_no_local_executor_stop_names_its_actions`), `task commission:conform` (`JudgePerformableActions`) |
| Reads proposed together (`ProposedReads`) are admitted only when the effect port reads every one and the handed frontier lists each `Admissible` with no authority, else nothing is invoked; admitted reads are revalidated each, invoked concurrently, each once through its binding, joined, and recorded in proposal order; a stale read alone is not invoked; an effect error fails the loop after every read joined; a fan-out is one step | `crates/loom-commission-testkit/tests/read_fan_out.rs` (`admitted_reads_run_concurrently_and_join`, `effects_are_recorded_in_proposal_order`, `a_fan_out_with_a_write_or_an_approval_required_read_invokes_nothing`, `a_stale_read_is_refused_and_the_others_are_performed`, `an_effect_error_fails_the_loop_after_every_read_joined`), `crates/loom-connectors/tests/read_fan_out.rs` (`each_read_is_invoked_once_through_its_binding`) |
| A consequential action leaves Loom only as a proposal, and the executor links no Connectors crate | `crates/loom-executor/tests/connector_boundary.rs` (`consequential_actions_leave_loom_only_as_a_proposal`) |
| A Connector invocation is made once, and only Connectors' own "nothing changed" is a refusal | `crates/loom-connectors/tests/connectors_invoker.rs` |
| A write through the Connectors command line runs only for an admitted request and is bound to its admitted action and arguments, with an absolute approval proof path passed and never read, and an idempotency key derived from the request id; another action, input other than the admitted arguments, an operation not described as a write, or a missing, relative or control-character proof path, invokes nothing; an `unknown` or lost write is returned unknown and never invoked again; reads still refuse a write; only an applied write of the admitted request becomes the protocol's evidence, naming its step and, for a route, its target (the `queue` or `owner` the admitted arguments name, which the governor removes before Canon reads the record); a route whose target cannot be read, or whose answer names another one, records nothing and says why | `crates/loom-connectors/tests/connectors_cli_write.rs` (`an_admitted_write_passes_the_proof_path_and_the_derived_key`, `a_write_without_an_absolute_proof_path_starts_nothing`, `an_operation_not_described_as_a_write_is_refused_before_the_invoke`, `the_same_admitted_request_yields_the_same_idempotency_key`, `an_unknown_write_is_returned_unknown_and_never_invoked_again`, `a_write_whose_answer_is_lost_is_unknown`, `invoke_read_still_refuses_a_mutation_operation`), `crates/loom-connectors/tests/support_triage_route.rs` (`an_applied_route_records_a_routing_record_naming_the_step`, `the_route_is_not_admitted_before_the_review_approved`, `a_route_not_applied_records_no_evidence`), `crates/loom-connectors/tests/adversary_w172_governed_write.rs` (`input_the_runtime_did_not_admit_is_never_written`, `a_request_admitted_for_another_action_writes_nothing`, `a_control_character_proof_path_starts_no_process`, `the_adapter_refuses_the_write_of_another_admitted_request`, `the_record_names_the_target_the_admitted_arguments_name`, `a_route_whose_target_cannot_be_read_records_nothing_and_says_why`, `an_answer_naming_another_target_records_nothing`), `crates/loom-governor/tests/adversary2_governor.rs` (`a_target_in_the_facts_leaves_the_record_as_canon_evaluates_it`) |
| A spec that cannot be served is refused before anything runs, naming the member and the reason, in declaration order of `loom.agent.CheckAgentSpec`; a credential in the document is refused without repeating it; `operator-reply@1` is in the plugin catalog, not the bundled one, and its `reply.send` is blocked until a reply is proposed | `crates/loom-agent/tests/check.rs` (`a_valid_spec_checks`, `a_state_naming_an_unknown_action_is_refused`, `a_source_without_a_binding_is_refused`, `a_secret_literal_is_refused`, `every_credential_shape_is_refused_without_repeating_it`, `a_credential_in_an_enum_member_is_refused_before_the_decoder`, `the_first_refusal_in_declaration_order_is_the_answer`), `crates/loom-cli/tests/agent_check.rs` (`a_secret_literal_is_refused_without_repeating_it`), `crates/loom-protocols/tests/operator_reply.rs` (`operator_reply_send_needs_a_proposal`, `plugin_catalog_lists_operator_reply`, `bundled_catalog_excludes_operator_reply`), `crates/loom-protocols/tests/inbound_answer.rs` (`bundled_catalog_excludes_inbound_answer`), `task conform` (`loom.agent.CheckAgentSpec`) |
| A served spec polls only `channel.allow`, sends only what it proposed, to the item's channel, at most `writes.max_per_hour` an hour, and in `writes.mode: approval` only after a person's grant; its authority is exactly `policy.capabilities`; SIGTERM ends the host after the item | `crates/loom-agent/tests/serve.rs` (`a_message_outside_the_allow_list_is_no_item`, `a_send_outside_the_allow_list_runs_no_command`, `a_send_without_a_proposal_is_not_admissible`, `a_sixth_send_in_the_window_is_refused`, `a_send_is_held_until_approved`, `the_spec_authority_grants_exactly_the_policy_capabilities`, `writes_mode_auto_is_refused_before_anything_runs`), `crates/loom-cli/tests/agent_serve.rs` (`sigterm_ends_the_host_after_the_item`, `agent_serve_once_holds_the_send_and_resume_performs_it`), `crates/loom-plugin-slack/tests/poll.rs` (`an_allow_list_restricts_the_walk_to_its_channels`), `task conform` (`loom.agent.AdmitSend`) |
| A plugin turn reads only the sources and kinds of its projection, and a proposal is recorded, never sent | `crates/loom-plugin/tests/effects.rs` (`effects_refuse_an_undeclared_read`, `propose_writes_the_record_only`) |
| A slack-handler run describes and invokes only the three configured Slack reads and the turns' data-source reads, and records one line per item it proposes for; nothing is sent | `crates/loom-plugin-slack/tests/run.rs` (`fixture_run_invokes_no_write`, `fixture_run_records_three_proposals`) |
| A read through Connectors is performed naming its audit record; a write without a recorded attempt is an error | `crates/loom-connectors/tests/connectors_invoker.rs` (`an_admitted_read_is_performed_with_its_result_and_names_its_audit_record`, `a_write_without_a_recorded_attempt_is_still_an_error`), `crates/loom-commission-testkit/tests/effect_invocation.rs` (`a_performed_that_names_another_record_than_its_binding_requires_is_not_answered`) |
| A session's change returns as a `ProposedAction` whose arguments are `{files}` only, a removed file's content null, and reaches the workspace only through the effect after revalidation; a proposal on a moved case is refused and never invoked; a workspace write made while the session runs is never proposed back, and one to a file the session changed (or added), or a link that now reaches a named path, suspends the step with `ExternalAvailability` instead of proposing it, as a named file not spelled as one plain relative path does before any session; each call mints a session id and records the session under the real commission and frontier ids; its summary and transcript digest ride on `ExecutorStep` and the step's observation, never as evidence; its usage is charged to the run; two sessions proposing the same files are the same request for the idle check | `crates/loom-session-executor/tests/proposes.rs`, `crates/loom-session-executor/src/lib.rs` (`a_named_path_a_link_reaches_since_the_copy_is_stale_not_gone`, `a_file_has_one_plain_spelling`) |
| A Codex session is recorded `Codex`, its every call decided by the executor, its home and XDG directories its own beside the copy, and its input charged less the cached tokens; one written by a codex outside the pin is held by H9 and suspends with `ExternalAvailability`; a b10x-harness session is recorded `Harness`, none of its calls decided, and charged the cost it stated and its calls' tokens summed, its input less the cached and cache-write tokens on every wire (b10x-harness states its input figure as the whole, those two parts of it), never below zero; a terminal usage total, where a session states one, is charged instead of the sum, never beside it | `crates/loom-session-executor/tests/kinds.rs` (`a_b10x_sessions_cache_writes_are_not_charged_twice`), `crates/loom-session-executor/src/lib.rs` (`a_terminal_usage_total_is_charged_once_and_events_only_without_one`) |
| A b10x-harness session given `SessionExecutor::api_key_secret` is launched under `ApiKey` with `--api-key-secret` and the name, no subscription token, key file or key variable; a name beside an endpoint token file or pointer, on a Claude Code or Codex session, or not `<namespace>/<name>`, or holding a control character, suspends the step with `ExternalAvailability` before any copy, naming it | `crates/loom-session-executor/tests/endpoint_secret.rs` (`a_named_secret_launches_the_loop_with_api_key_secret`, `a_secret_beside_a_subscription_token_is_refused`, `a_secret_for_a_vendor_session_is_refused`, `a_secret_not_namespace_slash_name_is_refused`), `crates/loom-session-executor/tests/adversary_w20261024w1_u1.rs`, `crates/loom-session-executor/tests/security_w20261024w1_u1.rs` |
| A b10x-harness session's reply is read per turn: a turn's `text-delta` fragments join with no separator, keeping the line breaks they carry, and each turn starts on a new line, so an action line streamed in many deltas is one line; Claude Code and Codex text events stay whole messages | `crates/loom-session-executor/tests/b10x_text_deltas.rs` (`an_action_line_split_over_deltas_is_read_as_one_line`, `a_turns_own_line_breaks_are_kept`, `turns_are_separated_by_a_line_break`), `crates/loom-session-executor/tests/adversary_w20261024w2_u1.rs` |
| A session built `continuing` an earlier session of its commission records it as `continues`, the next recorded session only, and is started with the request a fresh session gets; a predecessor of another commission (compared as UUIDs) suspends the step with `ExternalAvailability` naming it and both commissions, and a session id that is the predecessor's or already recorded suspends it naming the id, both before any copy or session, recording nothing; a session not built so continues none | `crates/loom-session-executor/tests/lineage.rs` (`a_continuing_session_names_its_predecessor`, `a_predecessor_of_another_commission_is_refused_naming_it`, `a_session_without_continuing_records_no_predecessor`, `the_request_of_a_continuing_session_equals_a_fresh_ones`), `crates/loom-session-executor/tests/adversary_w20261023w3_u2.rs` |
| A session's report lists each tool call's decision and who decided it, in record order, read from the `trace-ir/1` document of the events metaharness delivered; a record without a decision census, with a requested call it did not decide, or whose decisions disagree with any census count (each kind, `by_decider`) reports them unknown, never none; an empty tool name is no tool; they are never evidence nor in a proposal | `crates/loom-session-executor/tests/tool_decisions.rs` (`each_tool_decision_of_the_record_is_in_the_report`, `a_record_without_decisions_says_so`), `crates/loom-session-executor/tests/adversary_w20261022w3_tool_decisions.rs` |
| A confined child reads a host directory or file only where the host names it, read-only, at a mount Substrate does not own and no other mount overlaps, never the workspace, its bundle or the driver's state; a path that is absent, linked, of the wrong kind, not canonical or past 4 roots or 16 files is refused `ScopeInvalid` before launch, naming it; the record must name exactly the requested roots and files and working directory, and no writable root or list of apertures it did not ask for; `sandbox_path` maps only the workspace and the mounts | `crates/loom-intake-slice/tests/confined_child.rs` (`a_read_only_root_at_its_host_path_is_readable_and_not_writable`, `a_read_only_file_is_mounted_where_named`, `a_mount_under_a_reserved_path_is_refused_before_launch`, `overlapping_mounts_are_refused_before_launch`, `a_fifth_root_is_refused_before_launch`, `a_symlinked_or_absent_host_path_is_refused_naming_it`, `sandbox_path_maps_the_workspace_and_each_root`), `crates/loom-intake-slice/src/confinement.rs` (`a_record_naming_other_read_only_roots_is_refused`, `a_record_reporting_a_workdir_writable_roots_or_apertures_is_refused`) |
| A confined child starts in the working directory the host names, at or beneath `/workspace` or a read-only root's mount, and nowhere else: one outside them (`/scratch`, `/runtime` and `/` included), relative, with a `.`, `..` or empty component, not UTF-8, past 4096 bytes, at or beneath a read-only file, holding a control character (named escaped), absent, no directory, reached through a link or not searchable is refused `ScopeInvalid` before launch, naming it; a run whose output carries bubblewrap's `Can't chdir to` diagnostic for it is refused, never a program's exit; a driver without `exec.workdir` refuses it `CapabilityUnserved`; the applied confinement names it and a record naming another is refused; a child naming none keeps its wire, record and digest | `crates/loom-intake-slice/tests/confined_child.rs` (`a_child_starts_in_its_named_workdir`, `a_child_starts_in_a_workdir_under_its_read_only_root`, `a_workdir_outside_the_childs_mounts_is_refused_before_launch`), `crates/loom-intake-slice/src/confinement/workdir.rs` (`a_driver_without_the_workdir_fact_refuses_a_workdir_unserved`, `a_record_naming_another_workdir_is_refused`, `a_workdir_changes_the_digest_and_its_absence_does_not`, `a_workdir_holding_a_control_character_is_refused_escaped`, `a_workdir_the_process_cannot_search_is_refused_naming_it`, `a_run_bubblewrap_could_not_enter_its_workdir_is_refused`), `crates/loom-intake-slice/tests/adversary_w20261023w2_u2.rs` (`adversary_a_workdir_the_child_cannot_enter_is_refused_before_launch`, `adversary_a_child_that_cannot_enter_its_workdir_reports_no_exit_of_its_own`) |
| A confined child given up to four egress apertures reaches each by its host's name and no other host; the applied confinement lists every aperture, and a record naming another set, order or listen address is refused; the shared driver is keyed by the whole set in order; more than four, a name given twice, two apertures naming one host, and a later aperture naming a numeric host in any spelling `inet_aton` reads (hexadecimal, octal, fewer than four labels) or `localhost` are refused `ScopeInvalid` before launch, naming the apertures and their hosts; the first aperture admits no numeric host but `127.0.0.1` spelled so; an unconfined run's `network` is absent, never an empty list | `crates/loom-intake-slice/tests/confined_child.rs` (`a_child_reaches_each_named_aperture_and_nothing_else`), `crates/loom-intake-slice/src/confinement/several_apertures.rs` (`more_than_four_apertures_are_refused_naming_the_count`, `one_aperture_name_given_twice_is_refused_naming_it`, `a_later_aperture_naming_an_ipv4_literal_or_localhost_is_refused_naming_the_host`, `two_apertures_naming_one_host_are_refused_naming_it`, `a_numeric_host_in_any_spelling_inet_aton_reads_is_numeric`, `a_later_aperture_naming_a_numeric_host_in_any_spelling_is_refused_naming_it`, `the_first_aperture_allows_no_numeric_host_but_127_0_0_1_spelled_so`, `a_record_must_list_exactly_the_apertures_requested_in_order`, `a_driver_is_keyed_by_the_whole_set_and_declares_each_aperture`), `crates/loom-intake-slice/tests/adversary_w20261023w2_u1.rs`, `crates/loom-intake-slice/tests/security_w20261023w2_u1.rs` (`security_an_unconfined_run_does_not_report_no_network`) |
| The executor answers the commands of `ess/` as their synthesized scenarios specify, except `not-in-frontier`, which the conformance target answers; the session executor answers `loom.session.ProposeSessionChange` over a scripted Claude Code session | `task conform` (`crates/loom-conformance/tests/conform.rs`, `ess_conformance_report`); the membership rule: `crates/loom-executor/tests/adversary_run_revalidation.rs` (`not_in_frontier_follows_the_frontier_actions`) |
| The intake slice answers the commands of `ess/intake/` as their synthesized scenarios specify: a resume context is kept once (`KeepResumeContext`) and dropped (`DropResumeContext`), a merge hand-off is recorded once per request and per `HEAD` (`RecordMergeHandoff`), and the stores read back what the views `ResumeContexts` and `MergeHandoffs` publish | `task intake-conform` (`crates/loom-intake-conformance/tests/conform.rs`, `ess_conformance_report`) |
| The router refuses a pick outside the registry or below the threshold | `crates/loom-intake-router/tests/adversary_classify.rs` |
| A metered run reports one model call per `turn` of an llm `Model` wrapped in `MeteredModel`, failed turns included and a model wrapped again on its meter counted once, one step per executor call, holding the turns made on its thread or on another while it ran, and tokens and cost per step that sum to the run's, each turn priced with `metering::RateCard`; a cost no rate card priced is absent, never zero, as is the cost of a turn whose usage is not final or names no input or output tokens; the usage an executor that is not a model loop reports (`AgentExecutor::run_with_usage`) is its step's usage and no model call, never in the report's effects, and is in the meter only when the call came through `run_with_usage`, so the meter and the run's charges agree; an executor metered at its model port reports none, so nothing is counted twice | `crates/loom-executor/tests/run_metering.rs` (`the_reported_calls_equal_the_turns_the_model_port_saw`, `a_step_counts_the_selector_and_argument_calls_made_in_it`, `cost_is_absent_without_a_rate_card_and_for_an_unlisted_model`, `a_non_model_executor_usage_is_its_step_usage`, `a_step_sums_its_metered_turns_and_the_usage_its_executor_reports`, `an_executor_metered_at_its_model_port_reports_no_usage_and_is_counted_once`), `crates/loom-executor/tests/adversary_w11_u1_metering.rs` (`an_answer_whose_usage_is_partial_is_a_call_whose_cost_is_absent`, `a_model_metered_again_on_one_meter_counts_each_turn_once_however_it_is_reached`, `a_turn_on_another_thread_while_executor_calls_overlap_counts_in_the_last_opened`), `crates/loom-executor/tests/adversary_w20261020w4_u4.rs`, `crates/loom-executor/tests/adversary_w20261020w3_u3.rs` (`a_wrapper_that_does_not_forward_leaves_the_meter_and_the_budget_agreeing`) |
| An executor call's reported usage (`ExecutorStep.usage`) is charged to the run, never to an action or as a trial, and delivered in the step's observation; a usage with no cost leaves the run's cost absent; a charge past a spending limit suspends the Run with `RunBudget` once the step was observed, before its outcome is acted on | `crates/loom-commission-testkit/tests/executor_usage.rs` (`an_executor_usage_is_charged_to_the_run`, `a_run_budget_breach_by_executor_usage_suspends_the_run`, `an_unknown_cost_stays_unknown`) |
| A run budget's `max_tokens` bounds the input and output tokens charged to the run together, effects and executor calls, as `MaxTokens`; its `deadline`, read once as RFC 3339 before the Run starts (any other text is `LoopFailure::InvalidDeadline` and nothing starts, a resume taking nothing), is checked against `LoopContext::now` before each executor call and before a pending request is invoked again, and once an executor call was observed, after its spending limits, as a breach `on_breach` decides (by default nothing that call proposed is performed): at or after it the Run is suspended for `Deadline`, `max` the deadline's epoch seconds and `charged` the reading's, absent for a reading that is not RFC 3339, which counts as past; a pending request ends the run `Waiting`; a resume past it performs nothing, read before the take (the Suspension stays `Pending`) and right before the invoke (the Suspension released) | `crates/loom-commission-testkit/tests/run_and_action_budgets.rs` (`max_tokens_counts_input_and_output_together`, `a_passed_deadline_stops_the_run_before_the_next_step`, `a_deadline_that_is_not_rfc_3339_is_refused_before_the_run_starts`, `a_deadline_reached_while_a_request_is_pending_ends_the_run_waiting`, `a_deadline_passed_during_an_executor_call_discards_its_step`, `a_call_past_a_spending_limit_and_the_deadline_is_suspended_for_the_spending_limit`, `a_reading_that_is_not_rfc_3339_counts_as_past_the_deadline`, `a_deadline_with_an_offset_is_the_instant_it_names`, `a_resume_under_a_deadline_that_is_not_rfc_3339_takes_nothing`, `a_resume_past_its_deadline_performs_nothing`), `crates/loom-commission/src/rfc3339.rs`, `crates/loom-governor/tests/suspension_budget_limits.rs` (`every_budget_limit_reads_back_as_held`) |
| Under `StopPolicy.on_breach: KeepStep` an executor step whose usage passed a spending limit, or whose call ended at or after the deadline, is acted on as with no breach, its proposal revalidated and refused as any other; where the run would go on it is suspended for the breach before the next step, and a step whose outcome ends the run ends it with that outcome; `DiscardStep`, the default, suspends before the step is acted on | `crates/loom-commission-testkit/tests/run_and_action_budgets.rs` (`keep_step_acts_on_the_step_that_passed_a_spending_limit`, `keep_step_still_refuses_an_unadmitted_proposal`, `keep_step_performs_the_step_whose_call_ended_past_the_deadline`), `crates/loom-commission-testkit/tests/executor_usage.rs` (`a_run_budget_breach_by_executor_usage_suspends_the_run`) |
| `StopPolicy.breach_by_limit` chooses the step per limit, a limit it does not list taking `on_breach`: once an executor call was observed, every run limit it breached is collected (its spending limits in `spent_past`'s order, then the deadline, read whether or not a spending breach was kept), the step is discarded when any of them discards it and the Run suspended for the first that does, and otherwise acted on and the Run suspended before the next step for the first breached limit; a limit listed twice is `LoopFailure::BreachLimitListedTwice` before the Run starts, and a resume under it takes nothing | `crates/loom-commission-testkit/tests/stop_policy.rs` (`a_kept_token_breach_and_a_discarded_deadline_breach_each_take_their_own_choice`, `a_step_past_a_kept_and_a_discarded_limit_is_discarded`, `a_limit_listed_twice_is_refused_before_the_run_starts`, `a_resume_under_a_limit_listed_twice_takes_nothing`, `a_limit_not_listed_takes_on_breach`) |
| A host's stop (`LoopContext::host_stop`) is asked where a step would follow the admissibility and authority checks, right after the deadline and only when the deadline did not stop the run: before an executor call whose handed frontier admits an action without a grant, otherwise right before the effect of a request that call's proposal is admitted to, the provider's allow included; it suspends the Run for `HostStop`, carrying the host's name and detail, with nothing invoked (before the effect of a step kept past a breach, for that breach), ends the run `Waiting` while a request is pending, and on a resume takes nothing before the take and releases the Suspension right before the invoke, performing nothing; a frontier only a grant moves ends `NeedsAuthority` and one admitting nothing `NoAdmissibleAction`, the host not asked; the step budget, a kept breach, `NoPerformableAction` and the approval gate come first and the host is not asked; a passed deadline comes before a frontier admitting nothing; `FileSuspensionStore` and the conformance codec keep the reason as held | `crates/loom-commission-testkit/tests/host_stop.rs` (`a_host_stop_before_the_first_step_calls_no_executor`, `a_host_stop_after_one_step_stops_before_the_next`, `a_host_stop_while_a_request_is_pending_ends_the_run_waiting`, `a_host_stop_is_asked_once_before_each_step_and_never_after_the_run_ended`, `a_deadline_reached_first_is_the_reason_and_the_host_is_not_asked`, `a_stop_that_takes_no_step_comes_before_the_host_stop`, `a_frontier_only_a_grant_moves_ends_needing_authority_and_the_host_is_not_asked`, `a_granted_request_the_host_stops_is_never_invoked`, `a_frontier_admitting_an_action_without_a_grant_asks_the_host_before_the_executor`, `a_host_asked_before_the_executor_is_not_asked_again_before_the_effect`, `a_resume_the_host_stops_takes_nothing`, `a_resume_the_host_stops_before_the_invoke_releases_the_suspension`), `crates/loom-commission-testkit/tests/adversary_w20261022w2_host_stop.rs` (`adversary_a_kept_spending_breach_is_the_reason_when_the_host_stops_a_granted_request`, `adversary_a_kept_deadline_breach_is_the_reason_when_the_host_stops_a_granted_request`, `a_kept_steps_granted_request_the_host_does_not_stop_is_invoked_once`), `crates/loom-commission-testkit/tests/adversary_w20261022w1_host_stop.rs` (`adversary_a_frontier_admitting_nothing_ends_the_run_before_the_host_is_asked`, `adversary_a_passed_deadline_comes_before_a_frontier_admitting_nothing`), `crates/loom-governor/tests/suspension_host_stop.rs` (`a_host_stop_round_trips_through_the_file_suspension_store`) |
| A host's event sink (`LoopContext::events`, `EventSink`) receives each `RuntimeEvent` of a run once, in loop order, `Stopped` last and exactly once; a step that did not happen emits nothing, so a refused proposal emits `Proposed` and `AuthorityDecided` and never `EffectInvoked`; a failing provider is decided with no verdict; a resume emits its step, never its own checks, and a refused resume nothing; a run with a sink ends, invokes and observes exactly as one with `NoEvents`, and a panicking sink unwinds out of the loop | `crates/loom-commission-testkit/tests/runtime_events.rs` (`every_step_of_a_run_is_emitted_in_order`, `a_sink_sees_no_event_for_a_step_that_did_not_happen`, `a_failing_provider_is_decided_with_no_verdict`, `a_resumed_run_emits_its_request_and_stops_once`, `an_event_is_never_evidence_and_never_changes_the_loop`, `a_panicking_sink_is_the_hosts`), `crates/loom-sdk/tests/host_ports.rs` |
| A provider document Loom decodes itself, the Laya selector's answer, nested past 128 levels is refused; a model's answer is llm's to decode | `crates/loom-selector-laya/tests/laya_selector.rs` (`every_transport_or_answer_failure_is_unavailable`) |
| No hand-written type shadows an ESS-declared one | `task no-hand-model`, `task commission:no-hand-model` |
| Package and library names are the `loom-` names | `crates/loom-executor/tests/crate_names.rs` |
| No manifest names an archived Commission, governor or intake repository | `crates/loom-executor/tests/governor_import.rs`, `intake_import.rs`, `commission_import.rs` |
| The SDK stability page (`website/docs/reference/sdk-stability.md`) lists exactly the ports a host implements and the `loom_sdk` paths control-plane imports, each resolving through `loom_sdk`; a change to that surface updates the page and names any break in a `**Breaking:**` bullet of `CHANGELOG.md` | `crates/loom-sdk/tests/host_ports.rs` (`a_host_implements_every_port_through_loom_sdk_alone`, `the_stability_page_lists_exactly_the_ports_this_test_implements`, `every_control_plane_path_on_the_page_resolves`) |
| The `run --output jsonl` records and exit statuses change within `schema_version` 1 only by a new optional field, of a record kind or of a record one of its fields holds; any other change fails the gate until the committed contract moves | `task docs-check` (`crates/loom-docs/src/run_events.rs`, `the_v1_contract_matches_the_declared_events`, `a_removed_renamed_or_retyped_field_is_a_break`, `a_new_record_kind_is_a_break`, `a_new_optional_field_is_not_a_break`, `a_record_field_states_its_fields_each_judged_as_a_kinds_field`, `a_field_of_a_nested_record_the_page_leaves_out_is_named`) |
| A run whose agent is the session executor, wrapped by `EventStream::sessions`, writes one `Session` record per session, its fields equal to `metaharness::protocol::summarize` of the session's record and its digest the step's, and no `Turn`, `ToolCall` or `Usage` record for the session's own turns; Loom's own records are as without it; a record that does not project, and a summary whose cost is no finite number or whose count is past the largest integer, writes no summary field and says why in `projection_error`, which names no directory of the machine, and the run goes on | `crates/loom-cli/tests/session_events.rs` (`a_session_event_holds_the_summary_of_the_recorded_session`, `loom_s_own_records_are_as_without_the_session_record`, `a_record_that_does_not_project_is_named_with_why_and_fails_nothing`), `crates/loom-session-executor/tests/proposes.rs` (`the_session_record_is_summarized_by_metaharness`), `crates/loom-cli/tests/adversary_w20261022w2_u2.rs` (`adversary_a_token_count_past_the_largest_integer_is_never_written_as_another_count`, `adversary_a_projection_error_names_no_directory_of_the_machine`) |
| A release tag equals the workspace version and has a CHANGELOG entry | `loom-xtask release-check`, run by `release.yml` (`crates/loom-xtask/tests/release_check.rs`) |
| A release's CHANGELOG entry says what breaks for a consumer: a `**Breaking:**` bullet per break or the line `No breaking changes.`, never both, and no `**Breaking:**` bullet that names nothing or says `none`; a `Breaking:` sentence in prose does not count; `No breaking changes.` is refused when `consumers/pinned/`, `crates/loom-sdk/tests/host_ports.rs` or `docs/contracts/run-events-v1.json` changed since the previous tag; a `**Breaking:**` bullet is refused on a tag that moves only the patch version, and removing a path the previous tag's SDK stability page lists is refused unless an earlier release listed it under `### Deprecated`; `release.yml` fetches the history and tags it diffs | `loom-xtask release-check` (`crates/loom-xtask/tests/release_check.rs`: `release_check_names_an_entry_without_a_breaking_bullet`, `release_check_refuses_no_breaking_changes_when_a_stable_surface_changed_since_the_previous_tag`, `release_check_accepts_no_breaking_changes_when_no_stable_surface_changed`, `release_check_refuses_a_breaking_bullet_that_names_no_break`, `release_check_refuses_an_entry_with_both_a_breaking_bullet_and_no_breaking_changes`, `release_check_passes_on_the_first_tag`, `a_removal_without_a_deprecated_release_is_refused`, `a_breaking_entry_on_a_patch_tag_is_refused`, `release_workflow_verifies_the_tag_and_reuses_the_check`), `crates/loom-xtask/tests/adversary_w20261023w1_u4.rs` |
| The documented embedding builds and runs to `ApprovalRequired (repository.merge)` as an external crate: `consumers/pinned/` is its own workspace, depends on `b10x-loom-sdk` alone by path, includes `crates/loom-sdk/examples/software_change.rs` by `#[path]` and resolves one copy of each Loom and Canon crate; its library runs a `loom.agent/1` spec through `loom_sdk::Runtime` alone, and the run ends as the spec's budget and stop policy say | `task consumer-check` (`consumers/pinned/tests/embedding.rs`, `the_pinned_consumer_stops_at_approval_required`; `consumers/pinned/tests/agent_spec.rs`, `an_agent_defined_as_data_runs`), `crates/loom-xtask/tests/pinned_consumer.rs` (`the_agent_consumer_runs_a_spec_through_the_sdk_alone`) |

## ESS

Loom is specified in ESS under `ess/`. The domain is drafted and validated before any story
introduces a noun, and `task check` runs its conformance suite once synthesized. Change the
specification first.

The specification is a hard gate (Atlas ADR 0076). `task check` enforces it through
`task ess-gate`, which runs `crates/loom-executor/tests/ess_gate.rs`; a failure of any of the four
conditions fails `task check`:

1. `ess specify validate --path ess --strict-requires` exits 0;
2. `ess specify compile --path ess --format json` exits 0;
3. `ess verify conform synthesize` exits 0 with 0 refusals;
4. no file under `ess/` contains `UNMAPPED:`, so the specification carries no open question.

No story is implemented while the gate is red, whether or not it edits `ess/`. The `UNMAPPED:`
scan stands in for ESS until ESS refuses open entries itself: the `UNMAPPED:` scan is removed when
the ESS release that refuses open entries (beyond10x/ess `epic:typed-open-questions`) is pinned.

Commission's specification is its own ESS system under `ess/commission/`, held to the same four
conditions with `--path ess/commission` by `task commission:ess-gate`
(`crates/loom-commission/tests/ess_gate.rs`), which `task check` also runs. Intake's
`intake.routing` domain is `ess/intake/`, validated by `task intake-spec` and held to the suite
synthesized from it by `task intake-conform` (`crates/loom-intake-conformance/tests/conform.rs`),
which runs it against `b10x-loom-intake-slice`'s own stores over a real state directory and
reads the verdict from the report by Commission's rules. The governor's nouns are
Commission's and Canon's, except its stateless evaluation request, decision and refusal, which are
the `loom.evaluation` domain (`ess/domains/evaluation.yaml`), and its held case as a durable store
writes it, which is the `loom.governor` domain (`ess/domains/governor.yaml`); a noun it introduces
gets a declaration before a story is written around it.

An open question is settled before the specification changes, in a story or in a
`decision-blocker` when nobody has decided it, and is never written into `ess/` as an `UNMAPPED:`
marker.

Spec first, then red, then implement (Atlas ADR 0080). A unit's first commit changes only `ess/`;
on it a named test fails (a conformance scenario, `task drift`, or the story's own new test when
its declarations already landed), and the run is recorded; later commits make it pass without
changing `ess/`. Every story names that change and that test in its `## ESS first` section. Only a
change with no behaviour change is exempt, and its story says so.

`ess_gate.rs` (both copies) reads this section and fails when a phrase it checks is gone; reword
with the tests open. CI installs `ess` 0.69.0 (`.github/workflows/check.yml`); move that pin when a
newer ESS ships.

## Gate

Needs Rust, Task, `ess` and bubblewrap (`/usr/bin/bwrap`). CI installs the backend before
running the delegation-refusal tests. Run `cargo fetch --locked` before the gate, as CI does:
the offline dependency guard inspects the full graph, including platform-specific crates.
Before a change is reported done, in this order:

1. `task check`: the three specification validations, both ESS gates, all three drift checks, both
   no-hand-model checks, Loom's, Commission's and intake's conformance suites, the dependency guard,
   `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
   `cargo test --workspace --locked`, `task consumer-check`, then `task docs-check` and
   `task commission:docs-drift`. CI runs exactly this (`check.yml`, job `loom / task check`).
   `task consumer-check` copies the root `Cargo.lock` to `consumers/pinned/` and builds and tests
   that crate offline into its own `consumers/pinned/target/`; it is outside the workspace, so
   `cargo test --workspace` does not reach it. Beside it, job `loom / macos build` (`macos-15`) builds
   `b10x-loom-sdk` and `b10x-loom-intake-slice` with every target and runs their tests; the slice reads a
   process's end through `rustix::process::waitid`, because nix has `waitid` on Linux only. A test
   that needs Linux (confinement, `/proc`, a Linux program path) carries
   `#[cfg_attr(not(target_os = "linux"), ignore = "<why>")]`, so macOS still builds it.
2. `task plan` (`aep plan artifact validate`) when the planning store changed. `task check` does
   not run it.
3. `task website` when anything under `website/` changed, or a generated page did. CI's
   `Documentation validation` (`pages.yml`) builds the site and runs the `loom-docs` and
   `loom-commission-docs` tests.

There is no `rust-toolchain.toml`: CI builds on `stable` (`dtolnay/rust-toolchain`), and a local
toolchain newer than CI's `stable` can add Clippy lints CI does not have. Run the gate on CI's
version (`RUSTUP_TOOLCHAIN=<version> task check`) when Clippy disagrees.

`task install` builds and installs the current checkout into `~/.local/bin` through locked
`cargo install --path`; it does not fetch or move a branch. Run the gates before installing a
candidate for dogfooding.

`task --list` names every step, and each runs alone (`task drift`, `task conform`, `task commission:conform`, …).
`task commission:check` is Commission's own gate, limited to its crates. While iterating, prefer
`cargo test -p <package> --locked` on the crate you touched.

Every work tree builds into its own `target/`. The Taskfiles set no `CARGO_TARGET_DIR`; do not set
one. Two trees that share a build directory can run each other's test binaries: cargo judges the
other tree's binary fresh, and the gate exits 0 on code it did not build
(`gate_checks_the_tree_it_runs_in_when_worktrees_share_a_build_directory` in
`crates/loom-executor/tests/adversary_ess_gate.rs`). Before a gate's result counts as evidence,
check with `cargo test --workspace --locked -- --list` in the gated tree that the tests the run
printed exist there. Check `df -h /` before a full gate and run one full gate at a time. End a
managed worktree with `worktree finish --discard-cache --archive <tree>`, which removes its build
cache; never delete a `target/` by hand.

## Generated files

| Path | Generator | Regenerate | Drift check |
|---|---|---|---|
| `generated/rust/loom/` | `ess generate synthesize` over `ess/`, via `loom-xtask` | `task generate` | `task drift` |
| `generated/rust/commission/` | the same over `ess/commission/`, via `loom-commission-xtask` | `task commission:generate` | `task commission:drift` |
| `generated/rust/intake/` | synthesis over `ess/intake/`, via `loom-xtask` | `task intake-generate` | `task intake-drift` |
| `website/docs/reference/cli.md` | `loom-docs`, from `b10x_loom_cli::Cli` (`crates/loom-cli/src/lib.rs`) | `task docs-generate` | `task docs-check` |
| `website/docs/reference/crates.md` | `loom-docs`, from `cargo metadata` | `task docs-generate` | `task docs-check` |
| `website/docs/reference/ess/`, `website/data/ess/` | `loom-docs`, from `ess/` and `ess/intake/` | `task docs-generate` | `task docs-check` |
| `docs/contracts/run-events-v1.json` | `loom-docs`, the V1 contract of `run --output jsonl` from `ess/intake/` and `b10x_loom_cli::events` | `task docs-generate` | `task docs-check`, failing only on a break |
| `website/docs/reference/commission/` | `loom-commission-docs`, from `ess/commission/` | `task commission:docs` | `task commission:docs-drift` |

Never edit these by hand; change the source and regenerate. The generated Rust is never formatted:
`generated/rustfmt.toml` sets `disable_all_formatting`, so `cargo fmt --all` leaves it alone. Do not
add a `rustfmt.toml` inside a generated tree; `task drift` reports it and `task generate` deletes
it. A flag change in `b10x-loom-cli` regenerates the CLI reference. Every package needs a Cargo
`description`, and the crate page publishes it, so it names no story id or internal name.

`website/data/status.json` is maintained by hand. A change that ships, decides or drops a
capability updates it, the page that describes it and `CHANGELOG.md` (a bullet under
`## [Unreleased]`, § Releases) in the same commit. A change to a command, a crate or a rule updates README.md and this file in that commit too.

## Commission

Commission's history was merged into Loom, and its files keep their Commission paths so
`git log --follow` reaches it. Packages: `b10x-loom-commission` (the contracts and runtime, with
`run_until_blocked`), `b10x-loom-commission-testkit` (port fakes and adapter kits),
`b10x-loom-commission-conformance` (holds the contracts crate to its synthesized suite),
`loom-commission-docs` and `loom-commission-xtask`. A run suspended on a withheld approval
outlives its process only through the suspension port the host implements
(`ports::suspension`, `run_with_suspension`, `resume_after_grant`); `run_until_blocked` takes
none. `loom_sdk::Runtime` (`crates/loom-sdk/src/runtime.rs`) holds a run's ports, run store and
context and calls these three functions, returning what they return; it adds no behaviour, and
`crates/loom-sdk/tests/runtime_wrapper.rs` holds each method equal to its function. The port answers the Suspension's commands (`HoldSuspension`, `TakeSuspension`,
`ReleaseSuspension`), each atomically, decided by `decide_hold`, `decide_take` and
`decide_release`: a resume takes the Suspension before it performs anything and releases it after,
a `Resuming` one is in doubt and never taken again, and a commission holding one starts no new Run
through `run_with_suspension`. A host that already settled the request answers `HoldSuspension`
`settled` (`SuspensionSettled`), which `decide_hold` never answers: nothing is held, the runtime
puts the Run back `Running` (`ResumeRun`, so `run_with_suspension`'s run store implements it too)
and the run ends as without a port, with no loop error. The production store is the governor's
`FileSuspensionStore`.
`crates/loom-commission-testkit/tests/approval_suspend_resume.rs` holds it across a real process
restart. An effect port's `Refused` changed nothing, and an ordinary refusal is an idle iteration
toward the idle bound. One the port marks final (`EffectOutcomeRefused.ends_run`) ends the run at
once with `EffectRefused`, naming the request and the reason, once the refusal was recorded and
observed: after a proposal, after a fan-out once every read answered (beside a stale read it is
kept, the move is judged first, and it ends the run when the next load finds no move), and after a
resumed request, whose Suspension is released first. `outcome::judge_refusal` decides it with the behaviour ESS
generates for `JudgeEffectRefusal`; nothing is retried
(`crates/loom-commission-testkit/tests/refused_effect_outcome.rs`). Without a step budget, two
idle iterations in a row end the run `IdleBound`, naming them, as `outcome::judge_idle` decides
with the behaviour ESS generates for `JudgeIdleIteration`; when the last one's effect was
performed, the case is loaded first, as at the step budget, and a case the governor holds complete
ends the run completed. A frontier that admits nothing still ends it `NoAdmissibleAction`, and the
slice stops both `NothingAdmissible` (`crates/loom-commission-testkit/tests/idle_bound_outcome.rs`).
`LoopContext` is four parts a host may supply apart, `Ids`, `Clock`, `Limits` and `HostControl`,
and every type implementing all four is a `LoopContext` (`story:loop-context-split`,
`crates/loom-commission-testkit/tests/loop_context_split.rs`).
A host chooses both stops per run through `LoopContext::stop_policy` (`StopPolicy`): with
`approval_gate: false` no step arms the approval gate, so an unchanged frontier goes to the
executor and is judged idle, while authority is still asked and a withheld request is never
invoked; `idle_bound: Some(n)` is the bound `JudgeIdleIteration` judges against, with or without a
step budget, below 1 read as 1. The default, the gate on and no idle bound, keeps the stops above
(`crates/loom-commission-testkit/tests/stop_policy.rs`). Its `on_breach` says what happens to an
executor step whose usage passed a spending limit, or whose call ended at or after the deadline:
`DiscardStep`, the default, suspends the Run with `RunBudget` before the step is acted on; `KeepStep` acts on it as with no breach and suspends
the Run for that breach before the next step, unless the step's outcome ended the run.
Its `breach_by_limit` (`LimitBreachStep`) chooses per limit, a limit it does not list taking
`on_breach`: a step is discarded when any limit it breached discards it, the Run suspended for the
first that does, and kept otherwise; a limit listed twice is `LoopFailure::BreachLimitListedTwice`,
with nothing started and a resume taking nothing.
A run budget's `max_tokens` bounds input and output tokens together (`MaxTokens`), and its
`deadline` is an RFC 3339 instant, read once before the Run starts (`LoopFailure::InvalidDeadline`
otherwise, nothing started) and compared with `LoopContext::now` before each executor call and
before a pending request is invoked again; at or after it the Run is suspended for `Deadline`, and a
reading that is not RFC 3339 counts as past it. It is read again once an executor call was
observed, after the spending limits (kept ones too; only one that discarded the step skips it), and
a call that ended past it is a breach the stop policy decides. A resume reads it before the take, when a passed
one takes nothing, and right before the held request is invoked, when one releases the
Suspension; neither performs anything. The parser is Commission's own (`rfc3339`, no time
crate) (`crates/loom-commission-testkit/tests/run_and_action_budgets.rs`).
A host stops a run for a reason only it can read through `LoopContext::host_stop`
(`HostStop { name, detail }`, default `None`). The runtime asks it where a step would follow the
admissibility and authority checks, right after the deadline and only when the deadline did not
stop the run: before an executor call whose handed frontier admits an action without a grant,
before a pending request is invoked again, and on a resume. A frontier that admits no action
without a grant goes to the executor unasked, so the run ends as the authority or admissibility
check says (`NeedsAuthority`, `NoAdmissibleAction`), never `HostStop`; a request that call's
proposal is admitted to, the provider's allow included, is asked about right before its effect is
invoked. A stop ends the run as a passed deadline there does, with the Run suspended for
`SuspensionReason::HostStop` (`Waiting` while a request is pending) and nothing invoked; a stop
right before the effect of a step kept past a breach (`KeepStep`) suspends it for that breach,
which ends the run first, and a host that does not stop there leaves the kept step acted on. On a
resume, a stop before the take takes nothing and one right before the invoke releases the
Suspension. The step budget, a kept breach, `NoPerformableAction` and the approval gate stop the
run before the host is asked, and the host is not asked then. A passed deadline still comes before
a frontier that admits nothing. No effect is invoked past a stop the host names
(`story:host-stop-when-a-step-follows`). `FileSuspensionStore` keeps the reason
(`crates/loom-commission-testkit/tests/host_stop.rs`,
`crates/loom-commission-testkit/tests/adversary_w20261022w1_host_stop.rs`,
`crates/loom-commission-testkit/tests/adversary_w20261022w2_host_stop.rs`).
A host receives a run's events as they happen through `LoopContext::events`, an `EventSink`
(`ports::events`, `commission.responsibility.RuntimeEvent`): `StepStarted`, `SessionClosed`,
`ActionSelected`, `Proposed`, `AuthorityDecided`, `EffectInvoked`, and `Stopped` last and once for
every Run a loop started or resumed; only for a step that took place, so a refused proposal emits
no `EffectInvoked`. A resume's own checks before its Run resumed are no events. `NoEvents`, the
default, receives nothing and the loop builds no event for it. An event is never evidence or an
observation and never changes the loop; a sink that panics is the host's. `SessionClosed` names a
session by its transcript digest and never re-emits `metaharness.event/1` records (ADR 0096 point
7) (`crates/loom-commission-testkit/tests/runtime_events.rs`).
An effect port's `Pending` (`EffectOutcomePending`) says the work its request started has not
finished: the next iteration invokes the same request again instead of running the executor,
revalidated as before the first invocation, and a request whose case moved, or that the frontier
no longer admits or the provider denies, is not invoked again; one the provider answers
`ApprovalRequired` for ends the run `NeedsAuthority` and, under `run_with_suspension`, holds a
Suspension, so a resumed request answered `Pending` under the slice's `GrantAuthority`, which
allows only while the Suspension is `Pending`, asks for approval again. Each invocation is a step and one trial on
the run budget, each answer one observation; a pending answer is no idle iteration and leaves the
idle count where it was. A budget that stops the run while a request is pending ends it `Waiting`,
naming the requests and the budget's reason, as `outcome::judge_budget_stop` decides with the
behaviour ESS generates for `JudgeBudgetStop`; the Run is suspended for the budget as at any budget
stop, and the slice stops `StepBudget` (`crates/loom-commission-testkit/tests/pending_effect.rs`).
A `Pending` the port marks as ending the run (`EffectOutcomePending.ends_run`) is not invoked
again: once it was recorded, charged and observed, and no spending limit was passed first, the run
ends `Waiting` for `SuspensionReason::Time`, carrying the port's report, naming every request the
step left pending (a fan-out's in proposal order) and the Run suspended for it; a resumed request
so answered releases its Suspension first. Commission sets no timer: the host resumes the work.
A run whose effect port performs none of the actions it needs ends `NoPerformableAction`, as
`outcome::judge_performable` decides with the behaviour ESS generates for
`JudgePerformableActions`, naming those actions, each once, in frontier order, and the frontier's
open claims; the frontier relates no action to a claim, so the claims are the frontier's, and Loom
decides nothing about who performs them
(`crates/loom-commission-testkit/tests/unperformable_names.rs`). Design and contract notes are under
`docs/commission/`. Its tasks are in `Taskfile.commission.yml`, included under the `commission:`
namespace.

The frontier Loom reads is Commission's generated `Frontier`. Loom crates may depend on
`b10x-canon`; the Commission contracts crate may not.

## Governor

`b10x-loom-governor` (`crates/loom-governor`, library `loom_governor`) puts Canon behind the
governor and evidence ports Commission defines (ADR 0089). It decides and never acts: it evaluates
a case's protocol, reports the frontier and completion, and executes nothing. It is the only crate
that evaluates protocols with Canon, and it adds no clock, network or model call to an evaluation.
It depends on Commission by path, never the reverse.

Host-reviewed protocols enter through `CanonGovernor::with_protocol`; compile and frontier
representability checks remain in the governor. `EvaluationTime` is a trusted host callback,
never a model argument. Durable stores implement `FallibleCaseStore` atomically and report
failures explicitly; the legacy `CaseStore` bridge is only for infallible adapters.
`FileCaseStore` is one: a file per case, written as `loom.governor`'s `StoredCase` to a temporary
file and renamed, under a per-operation lock; `crates/loom-governor/tests/restart_durability.rs`
holds a case across a real process restart. `FileSuspensionStore` is Commission's suspension port
over the same lock and write (the private `locked_dir` module), a file per commission, so the two
may share one directory; the testkit's `FileSuspensionStore` re-exports it, and
`crates/loom-commission-testkit/tests/approval_suspend_resume.rs` holds both in one directory
across a restart. `FileWorkflowRunStore` is the governor's `WorkflowRunStore`, the port a
`WorkflowExecutor` keeps its run's `loom.workflow.WorkflowRunRecord` through, over the same lock and
write, a file per Run (`workflow-run-…`); a file holding another Run's record is refused, and
`crates/loom-commission-testkit/tests/workflow_run_store.rs` holds a workflow run across real process
restarts. The port and its `RunStoreError` are `b10x-loom-executor`'s (`workflow`, beside
`WorkflowExecutor`), so the governor depends on the executor and never the reverse. The host
restores the same admitted protocol definitions on recovery and authenticates evidence before
`submit_evidence`. The governor supplies no authority decision. The explicit decisions
(`canon-decisions/1`) it supplies are the ones the host recorded with `CanonGovernor::decide`, each
at the case revision current when recorded; `evaluate` takes them in the request's `decisions`.
Canon judges them, and Loom decides nothing about who may take one
(`crates/loom-governor/tests/explicit_decisions.rs`). A case completes only when exactly one
declared outcome is legitimate, unless the host builds the governor with
`CanonGovernor::with_completion_rule(CompletionRule::FirstDeclared)` (`loom.governor.CompletionRule`):
then it completes on the first legitimate outcome in the admitted definition's declaration order,
kept at admission because Canon's compiled protocol and decision document key outcomes by id, and
never on one Canon holds blocked. The rule is host configuration, never read from a case or a
model and not stored with a case; `evaluate` keeps the default
(`crates/loom-governor/tests/completion_rule.rs`).

`evaluate` (`loom_governor::evaluate`, `b10x-loom evaluate`) decides a caller's `canon-case/1`
snapshot and `canon-evidence/1` records under a protocol of the host catalog, never raw YAML from
the caller. Its request, decision and refusal are the `loom.evaluation` domain
(`ess/domains/evaluation.yaml`); its signature names no Commission type. It goes through the
private `decide_case` and `project` that `CanonGovernor` uses, never a second evaluator, and a
refusal names the input (protocol, snapshot, evidence record by position, time). A readable
record that does not apply is set aside as `CanonGovernor` sets it aside; an unreadable record or
a repeated id is refused. The time and the records are judged once each, never by re-running Canon
on prefixes. `crates/loom-governor/tests/evaluate.rs` and `evaluate_adversary.rs` hold it against
`CanonGovernor`; `crates/loom-cli/tests/evaluate.rs` holds the subcommand against the library.
`CanonGovernor::decision_document(case, frontier)` returns the whole `canon-decision/1` document
behind a frontier id the governor issued, as the generated JSON value `evaluate` returns in
`canon`: `frontier` keeps the document under the id it issues, in memory, so a host holding an id
from Commission's loop (`LoopEnd::last_frontier`) reads that frontier's document whatever the
trusted time did since. A lookup never evaluates; it answers `None` for an id the governor did not
issue or no longer keeps. Per case it keeps only documents for the case as currently held, at most
`KEPT_DOCUMENTS_PER_CASE` (16), dropping them when it finds the held case changed or the store no
longer holding it, and refuses a case as `frontier` does
(`crates/loom-governor/tests/decision_document.rs`, `adversary_w20261020_u3.rs`).

A case opens with the value of every case input its protocol declares (`CaseInputs`;
`software.change/1` declares `risk`, `change_kind` and `affects_security_boundary`), given by the
host as trusted context, never by a model. `open` and `open_case` refuse an input the protocol does
not declare (`UndeclaredInput`), a value it does not list (`UnlistedInputValue`) and a declared
input left out (`MissingInput`), each naming the input, before anything is stored; a protocol that
declares none opens with `CaseInputs::none()`. `FileCaseStore` keeps them in `StoredCase.inputs`,
and a case stored without them (as 0.19.0 wrote every case) is refused as a store failure naming
the case, never filled with defaults. `CaseInputs` refuses one input given twice
(`RepeatedInput`), never replacing the first. Every read of a held case checks its inputs again as
at opening: a case whose stored inputs do not satisfy its protocol (an empty list included) is
never decided and its inputs are never reported (`GovernorUnavailable`), and `FileCaseStore`
refuses such a case of a built-in protocol naming the case and the input
(`crates/loom-intake-slice/tests/adversary_w9_u1_inputs.rs`,
`a_stored_case_with_unusable_inputs_is_never_decided`). An obligation is `open` only when Canon
reports it `open`: one Canon reports `not_applicable` (the case's inputs do not select it) or `discharged` is not.

Canon is named by the reference `b10x-canon-engineering` uses (`tag = "0.6.0"`), pinned by
`Cargo.lock`. A different reference builds a second Canon whose types do not match. Move Canon with
`cargo update -p b10x-canon -p b10x-canon-engineering` together with the `b10x-canon-engineering`
tag (now `0.9.0`); any other crate that adds Canon uses the same reference.

## Connectors

`b10x-loom-connectors` (`crates/loom-connectors`, `loom_sdk::connectors`) is the one crate that
links Connectors: `connectors-client` and `connectors-core` at tag `v0.35.0`. Move both together.
`ConnectorsInvoker` fills Commission's `ConnectorInvoker` from the host's `ConnectorEndpoint`
records (`ess/commission/domains/responsibility.yaml`), one per instance, and a
`CredentialResolver` the host injects; it reads no credential itself. It describes on
`GET /v1/describe` and invokes once on `POST /v1alpha2/invoke`, never `/v1/invoke`, and never
resends. A binding declares its operation's `effect`: `Write` for the `mutation` profile the
service describes (Connectors records an attempt), `Read` otherwise; a binding described otherwise
is `Err` and not invoked. A write's `Performed` names its attempt, a read's the execution audit
record Connectors completed (`audit`) and no attempt, and `ConnectorEffects` refuses any other
`Performed` as a failure to answer. `Refused` is only what Connectors states as nothing changed
(`mutation.classification` `refused` or `not_attempted` on a valid response); every other failure,
a write without a recorded attempt included, is `Err`. The sync port runs on a Tokio runtime the
invoker owns. `crates/loom-connectors/tests/connectors_invoker.rs` holds it against a fake service
on a loopback port.

`loom_connectors::cli::ConnectorsCli` is the other path: a client over the operator's
`connectors` command line, for reads and for one governed write. As a read client it serves a host polling a `loom.datasource` source outside a run and a
plugin turn's `source.read` inside one. A read needs no `AdmittedRequest` and no binding. Each read is
`operations describe`, then exactly one `operations invoke --input-stdin`, with `--output json` and
the configured `--config` and `--state-dir`; argv holds only those flags and their ids. It reads
no credential: the CLI's own keyring custody holds it, and the program starts with only the
variables `cli::INHERITED` names, never a token. Writes are refused twice. The client refuses an
operation `operations describe` reports with the `mutation` profile (`cli::WRITE_PROFILES`) before
any invoke; `describe` prints no effect, and adapters publish other profiles, `resource` among
them, for reads, which are read. Connectors refuses any write without `--approval-file` (connectors
`v0.38.0`, `crates/connectors-host/src/local/owner/mutation/execution.rs:161`), which no read
passes (`no_invoke_ever_carries_an_approval_file` holds `src/cli.rs`, where the reads are); that second check is what holds a write an
adapter publishes under another profile. A kind the source declares no operation for starts no
process. Each command is bounded by the configuration's `timeout_seconds` (60 when absent) and the
process it started is killed and reaped past it; a process that process forked is not killed, and
outlives the read. A timeout this host's clock cannot hold starts no command and fails naming it
(`a_timeout_too_large_for_the_clock_starts_no_process`). A Connectors refusal is a
`ReadRefusal` naming the source, never retried; `not_granted`'s names the remedy, `connectors
connections revalidate --adapter <alias>`, which the client never runs.
`crates/loom-connectors/tests/connectors_cli.rs` and `connectors_cli_adversary.rs` hold it against
the fake `tests/fixtures/connectors/fake-connectors`, which logs its argv.

`ConnectorsCli::invoke_write` (`src/cli/write.rs`) is the local transport for a write the runtime
admitted; `ConnectorsInvoker` stays the service transport. It takes the `AdmittedRequest`, a
`loom.datasource.WriteOperation` and a `WriteApproval` naming the absolute path of the proof
`connectors approvals issue` wrote for the person's confirmation: Loom passes the path as
`--approval-file` and never reads, writes or stores the proof; an empty or relative path, or one
holding a control character, is refused before any process starts. The write is bound to the
admitted action and arguments: an operation whose `action` is not the admitted request's is refused
`action-not-admitted`, and input other than its admitted arguments `input-not-admitted`, before any
process starts. The describe must report the `mutation` profile, else `not-a-write` and
no invoke. The invoke carries `--idempotency-key` from `cli::idempotency_key`, derived from the
admitted request id, and runs once; its answer is a `WriteResult` with Connectors'
classification, and an answer lost or unreadable after it started is `Unknown`, never retried.
`loom_connectors::evidence::WriteEvidenceAdapter` is the host evidence adapter: only an `applied`
write of its action becomes the protocol's evidence (an observation, then `submit_evidence`),
naming the action and the admitted request, and the write of another admitted request (another
idempotency key) records nothing. A route's record names its target in its facts (`target`): the
`queue`, or for an owner route the `owner`, the admitted arguments name, exactly one of them, as
non-empty text; a target that cannot be read, or an answer from Connectors naming another queue
or owner, records nothing, and `WriteEvidenceError::Unreadable` says why. The governor removes
`target` before Canon reads the record, as it removes a facts `producer`.
`tests/connectors_cli_write.rs` and `tests/support_triage_route.rs` (on `support-triage@1` and
the Canon governor) hold it.

## Mandate

`b10x-loom-mandate` (`crates/loom-mandate`, library `loom_mandate`) is the one crate that links
Mandate: `mandate-authz`, `mandate-types`, `mandate-model`, `mandate-graph` and `mandate-policy`
at tag `v0.9.0`. Move them together. `MandateAuthority` implements Commission's
`AuthorityProvider` over `mandate_authz::check`, and does not read the action request
Commission passes beside the capability; its builder takes every Mandate port (graph,
policy, role catalog, challenge issuer, delegation records, decision ids), the tenancy and
topology, and the host's audience, relation, minimum revision, attributes and clock; it configures
no policy. The subject is Mandate's verified context, which the host puts in the commission's
authority context as a `loom.authority.HostContext` (`{"verified": …}`); the action is the
capability; the resource is the case, of type `loom.case`. A case id is opaque (Loom's governor
issues `case-<n>`), so its Mandate resource id is derived, never parsed: the UUID version 5 of
the case id's exact UTF-8 bytes under the namespace `735b2acd-620b-43ec-b1ab-144c2aec7ba7`
(`loom.authority.CaseResource`). Two distinct case ids, two spellings of one UUID included, are two
resources. A host computes a case's resource id with `loom_mandate::case_resource_id(&CaseId)`,
or any UUID version 5 implementation over that namespace, when it writes Mandate's graph or policy;
`the_case_resource_is_derived_as_the_specification_declares` holds the namespace to the
specification. The `loom.authority` domain (`ess/domains/authority.yaml`) declares each outcome
and the answer it maps to, as the `answer` attribute of `loom.authority.Outcome`; ESS synthesis
generates no accessor for it, so `loom_mandate::answer` writes it once and
`every_outcome_answers_as_the_specification_declares` holds the two together. Mandate's
`Unavailable`, an authority context that does not decode, the empty case id (which derives no
resource id) and a contradictory answer are an `Err`, which Commission refuses. It is opt-in:
`b10x-loom-cli` alone links it, behind `--mandate` (below); `b10x-loom run` does not use it.

`loom_mandate::approval` (`story:mandate-approval-on-resume`) clears a resume's approval through
Mandate v0.9.0's `check_with_approval`. Mandate records no approval itself, so Loom keeps one:
`approve --mandate <FILE>` appends a Mandate `Approval` of the held request (by the file's
approver, expiring `approval_seconds` later) to the state directory's approval event log
(`mandate-approvals.jsonl`, `intake.routing.MandateApprovalEntry`,
`b10x_loom_intake_slice::mandate_log`) and the grant names it (`Grant.mandate_approval`).
`resume --mandate <FILE>` lets `GrantAuthority` allow the held request only when
`HostedMandate::clear` (over every log entry, folded as Mandate's own delegation fold) answers the
`ApprovedDecisionRecorded` event, which is appended before anything is performed, under the
directory's lock: a crash after it leaves the request unperformed, never performed twice, and the
same approval is refused `wrong-state` afterwards. `FILE` is host-trusted
(`loom.authority.MandateHost`): the verified context, the approver (neither the verified subject
nor its actor), the approver policy, the relation, the capabilities and those needing an approval;
Loom builds Mandate's own doubles from it for the case until Mandate ships a policy adapter. The
commission and request an approval is bound to are checked on the record Mandate reads: found by
the approval's parsed UUID, never its text, and the last one recorded, as Mandate's lookup takes
it. A grant naming no approval is refused under `--mandate`; only Mandate clears a grant naming
one, so `resume` without the flag refuses it `NotGranted` and performs nothing. Otherwise, without
the flag, `approve` and `resume` are as on 0.20.0.
The flag and the log go when Mandate records approvals itself. `loom_mandate`'s source opens no
file: the slice's `mandate_log` keeps the log, and the command line (`b10x_loom_cli::mandate`)
joins the two. Since this story `b10x-loom-cli`
links `b10x-loom-mandate`, the one crate that does (the SDK does not), and `approve` and `resume`
use it only behind `--mandate`; `b10x-loom run` does not use it. Over the closure of `Cargo.lock`,
only `b10x-loom-cli` and `loom-docs`, which links it for its help text, reach Mandate.

## Agent spec

An agent as data is the `loom.agent` domain (`ess/domains/agent.yaml`, `story:agent-domain`):
`AgentSpec`, the typed `loom.agent/1` document, with its `ToolPolicy` (tools allowed and denied,
egress apertures and the metaharness `DecisionMode`, `frame`, `ask` or `observe`). `model` is
exactly `loom.models.ModelsConfig`. Its run budget and stop policy mirror Commission's
`RunBudget` and `StopPolicy` field for field, because an ESS system cannot name another system's
type; a change to either changes both. `bindings` is a union tagged `kind` (`connectors`, `http`),
and a Kubernetes selector is a list of `Label`s, ESS having no map. ESS generates no decoder, so
`loom_sdk::agent::parse` (`crates/loom-sdk/src/agent.rs`) decodes YAML or JSON into the generated
types, refusing an undeclared member, a missing required one, a wrong type, an undeclared enum
spelling and a `format` other than `loom.agent/1`, each naming the member; the models section goes
through the models configuration's own decoder. It checks the shape only. Whether a spec can be
served is `b10x-loom-agent` (`crates/loom-agent`, `check` and `check_text`, run by `b10x-loom agent
check --spec`): the ESS command `loom.agent.CheckAgentSpec` with its error `SpecRefused` (`reason`,
`member`, `detail`) and the enum `RefusalReason`. Against a closure answering a plugin-catalog
protocol's actions, the first refusal is the answer. `check_text` drops one leading byte order
mark, then: `Malformed` (`document`, the decoder's message) when the text is not one YAML document
the decoder reads (a scan error, an empty document, several documents, past its byte, flow or
alias bound); `SecretLiteral` (a URL with user information or a query, or a token prefix of
`TOKEN_PREFIXES`; the value is never repeated), found by `credentials.rs` walking the YAML the
parser reads, every scalar, key and tag in document order, before the document is decoded, so it
covers every member by construction and precedes the decode-level `Malformed`: a decoder message
(an undeclared enum spelling, an unknown key) never carries a credential; a key holding one is
named by the mapping that holds it. Then `Malformed` (the decoder refuses the document), and
`check`'s own checks in the declaration order of the command's outcomes: `UnknownProtocol`,
`UnknownAction`, `UnboundSource` (states, then the outer loop, then the channel), `HttpMethod`,
`SendWithoutWrites` and `EmptyChannelAllow`. `check` takes a decoded spec and so starts at
`UnknownProtocol`; a credential is the text's to refuse. The walk reads no more than the decoder:
on a document the decoder refuses it runs under the decoder's own byte, flow and alias bounds
(`the_credential_walk_keeps_the_decoders_bounds`). The conformance target plays
the file system for the command's `path`: a file under the repository root is read as it is, a
synthesized path reads the fixture of the forced branch (`ess/scenarios/agent-check/specs/`) or,
forced nothing, the operator example. The scenarios are authored `ess-scenario/4` documents under
`ess/scenarios/agent-check/`, listed in `ess-inputs.yaml`, their names hyphenated
(`a-valid-spec-checks`, `a-state-naming-an-unknown-action-is-refused`), since ESS admits nothing
else (`ESS-AUTHOR-001`). `examples/agents/operator.yaml` decodes
(`crates/loom-sdk/tests/agent_spec.rs`, `the_operator_example_decodes`; the unknown-member unit test
is `agent::tests::a_spec_with_an_unknown_member_is_refused_in_yaml_and_json`). It also refuses
empty or blank text, a YAML tag on any value, a `bearer_env` that is no environment variable name
(`[A-Za-z_][A-Za-z0-9_]*`, the text never repeated) and a port outside 1 to 65535. Before reading
YAML it refuses a document holding more than `MAX_FLOW_OPENERS` (4096) `[` or `{` bytes, counted
anywhere, because the YAML reader's cost grows with the square of flow nesting
(`crates/loom-sdk/tests/adversary_w20261022w3_agent_domain.rs`). The types of other domains a spec
holds (`Decimal`, `Timestamp`, `SessionKind`, `Intent`, the datasource ids and
`ConnectorsCliConfig`) are re-exported from `loom_sdk::agent`.

Serving a checked spec is `SpecPlugin` (`crates/loom-agent/src/serve.rs`, `story:agent-serve`), the
`Plugin` `b10x-loom agent serve --spec --state [--once]` runs through `run_plugin`. `SpecPlugin::new`
runs `check` again, needs `runtime`, `loops` and `channel`, a `connectors` channel binding with
`list`, `get` and `search`, and refuses `writes.mode: auto` (not served yet); it lowers the spec
to the host's `PluginConfig` (connectors from `runtime`, sources from the `connectors` bindings,
the outer loop's interval and the inner loop's threshold) and to a `SlackHandler` over the channel
binding, `with_channel_allow(channel.allow)`, so a message outside the allow list is no item. The
projection of an item is the state whose `when.intent` is its intent (its actions, and its sources
among the configured ones); no state means a `stopped` record. A turn runs `turn_with` on the inner
loop's protocol with the spec's budgets, over a `FileCaseStore` and `FileSuspensionStore` in the
host's state directory, under `SpecAuthority` (grants exactly `policy.capabilities`, holds
`reply.send` as `ApprovalRequired` in `writes.mode: approval`, denies everything else) and a
`SendPolicy` for the channel binding's `write`. A send that is held leaves the Suspension in the
state directory and `approvals/<commission>.json` naming the item; the record line says
`reply.send awaits approval: commission <id>`, the commission being `commission_id(agent, item,
revision)`, a stable UUID of the three. `b10x-loom approve <COMMISSION> reply.send --state <DIR>`
writes the grant as it does for a run; `SpecPlugin::resume` performs the send once through
`resume_after_grant` under `GrantAuthority`, with the grant file as the write's proof, and a serving
host does so for every granted commission at its next poll (`b10x-loom agent resume` does it now).
Sends are counted in `sends.jsonl` in the state directory. `agent serve` settles the models before
any read: the spec's `model` section (`loom_sdk::models::from_routes` on `model.routes`, relative to
the spec, for the `agent` and `classifier` roles) or `--catalog` with `--model` and
`--classifier-model` as `plugin run`. The tests are `crates/loom-agent/tests/serve.rs` (the
plugin, the authority, the admission and the approval flow on the fake `connectors`) and
`crates/loom-cli/tests/agent_serve.rs` (the binary, `sigterm_ends_the_host_after_the_item`).
No live run against a channel was made for this story; `docs/qualification/2026-10-25-agent-serve.md`
says what one is.

## Plugin host

`b10x-loom-plugin` (`crates/loom-plugin`) hosts a plugin (ADR `plugin-hooks`): a crate linked at
build time implementing `Plugin` (`poll`, `classify`, `project`, `turn`, `result`, `objectives`),
run by `run_plugin(plugin, config, state, stop)`. Its nouns are the `loom.plugin` domain
(`ess/domains/plugin.yaml`); a plugin's own configuration extends `PluginConfig`. The plugin hooks
sit outside a run and grant nothing. `ProtocolCatalog::plugins()` holds `inbound-answer@1` and
`operator-reply@1` (`inbound-answer@1` plus `reply.send`, a write under the `reply.send`
capability once a reply is proposed); `bundled()` holds neither. A turn is a thin caller of
`run_until_blocked` through `loom_sdk`, on `inbound-answer@1` from `ProtocolCatalog::plugins()`,
with Loom's chooser path (a
`Loom` over `b10x_loom_executor::model_selector`'s `ModelSelector` and `ModelArguments`, on the
turn's llm `Model`, revalidating against the governor), `PluginAuthority` (grants `datasource.read`
and `reply.propose`, denies every other capability) and `DataSourceEffects`, which reads through
`loom_connectors::cli`, refuses a `{source, kind}` outside the projection before any command, and
submits one `source_read` evidence per performed read. `turn_with` takes `TurnOptions` (the
protocol, the authority, the case store, an optional suspension port, an optional `SendPolicy` and
commission, the budgets) and answers `Turned` (the result, the commission, whether a send awaits
approval); `turn` is the read-only default over them. With a `SendPolicy`, `DataSourceEffects`
performs `reply.send`: `admit_send` (`effects.rs`, `loom.agent.AdmitSend` in its declared order:
`writes-none`, `outside-allow-list`, `no-proposal`, `rate-limited` are final refusals; `held` ends
the step refused but not the run; `admitted` writes) decides before any command, the arguments must
name the item's channel, its `ts` as the thread and the proposed text verbatim, the write goes
through `ConnectorsCli::invoke_write` with the policy's proof file, and one `reply_sent` evidence
names what Connectors answered. The send counter is `sends.jsonl` under the policy's directory, one
line per performed send, read within `SEND_WINDOW_SECONDS`. Each step offers the frontier's catalogue,
narrowed to the projection's actions, in one forced `select_action` request, then asks for the
selected action's arguments in one forced `action_arguments` request whose schema is the action's;
both carry what the turn did so far, the reports of its reads included, as one data block
(`<earlier-steps>` around a JSON list of strings, `<`, `>` and `&` written as `\u` escapes) no value
can close, introduced as data and never as instructions; every other value a request quotes (the
item's text, id and revision, what `operations describe` reports) has those escaped too, so none
opens or closes a block (`crates/loom-plugin/tests/earlier_reads.rs`,
`a_later_step_sees_an_earlier_read_as_data`, `no_value_a_turn_quotes_spells_the_data_block`). A
turn is one Run, bounded by `TURN_STEP_BUDGET`: a step Commission performed under a grant passes
its approval gate (`docs/commission/design/approval-gate.md`), so the Run goes on after each
granted read, and an `AwaitingApproval` ends the turn `stopped`. Model requests are bounded per
turn: `TURN_MODEL_BUDGET` (two per step) counts every request sent, each attempt of Loom's caller
retries (`loom_sdk::intake::slice::selector::send_with_retries`) included; the model the turn lends
refuses any past it. A model that answered a request without a usable call is about the item, and
one that could not answer is not: an answer with no call of the forced tool (a provider's refusal
or content filter, which llm decodes as `Incomplete`, or text only), an answer llm refuses for the
request, a selection naming no action and arguments Commission's JSON cannot carry
(`model_selector::AskError::Unusable`) end the Run as no useful action, with no further request,
and the turn `stopped`, naming the answer, so the item is recorded and the cycle goes on; only a
request never answered (it could not be sent, or the model failed it after every retry;
`AskError::Unanswered`) is `PluginError::Unavailable`. A selection outside the candidates stays a
selection error, never a selection (`crates/loom-plugin/tests/unusable_answer.rs`,
`crates/loom-plugin/tests/adversary_w20261020w4_u3.rs`). A classifier hint that
names no configured source is dropped before anything prints it. Nothing is sent: a reply is a
proposal in the record. A task gets no turn: the router's pick from `ProtocolCatalog::bundled()`
is recorded as a proposed case. An attempt on an item is counted in the state, which keeps the
item, before it starts; a classify or turn hook that fails leaves it unhandled, each cycle
retries the failing items from the state before it polls, and the third attempt that does not
succeed records it `stopped`. A failure that is not about the item (`PluginError::Unavailable`:
sources that cannot be described, a read whose connectors program timed out or that Connectors
refused `not_granted`, a model that cannot be reached, the router's model or catalog) counts
against none and ends the cycle without saving the poll's cursors. A poll that fails is such an
outage of the whole host: the cycle saves no cursor and the host polls again after its interval;
`run_plugin` returns the poll's failure only when `stop` ends it after that cycle (`once`). A
`stop` set while the host runs ends it after the item being handled, leaving the poll's other items
and its cursors to the next host; a `stop` already set at start (`once`) runs its whole cycle
(`crates/loom-plugin/tests/host.rs`, `a_poll_outage_does_not_stop_the_host`,
`a_stop_ends_the_host_after_the_current_item`). The state directory
holds `state.json`, written to a temporary file and renamed, and `record.jsonl`, the source of
truth for handled items, whose torn last line is dropped under the lock. One host holds it at a
time: `run_plugin` takes an exclusive lock on the record before anything else, and a second host
is refused once the lock stayed held for `LOCK_WAIT`. One inside a configured workspace root or
checkout is refused by comparing paths, with no git command. The tests in
`crates/loom-plugin/tests/` use a recorded classifier, a scripted turn model answering each step's
selection and arguments, and the fake `connectors` of `loom-connectors`. The command line hosts registered plugins through
`b10x-loom plugin run|report` (§ Slack handler).

## Slack handler

`b10x-loom-plugin-slack` (`crates/loom-plugin-slack`) is the slack-handler plugin, the one
`b10x-loom plugin run` registers (`PluginName`, held to the crate's `NAME` by a unit test of
`b10x-loom-cli`). Its configuration is the `loom.slack` domain's `SlackConfig`
(`ess/domains/slack.yaml`), which holds the host's `PluginConfig`; `config::read` decodes it from
JSON, refuses an unknown member, and `config::check` refuses one without the bot's user id, the
Slack adapter or connection before anything runs. Its own hook is the poll; classification,
projection, the turn and the record are the host's defaults. The poll reads only through
`ConnectorsCli::invoke_read` on the configured adapter and connection, with the three configured
read operations, and never a write: the channel list, each member channel's history from its
stored `ts`, and a thread only for a message with replies. The walk orders channels by objective
weight, staleness, member count, then a seeded draw. A message younger than `min_age_minutes`, a
bot's, a system message (`subtype` other than `file_share`), one the bot reacted to or one with a
thread reply from anybody but its poster is no item; mentions of the bot come first. A channel's
cursor never passes a message too young to judge. `SlackHandler::with_channel_allow` restricts the
walk to the member channels it names (`an_allow_list_restricts_the_walk_to_its_channels`). The tests in `crates/loom-plugin-slack/tests/`
reuse `crates/loom-plugin/tests/support/mod.rs` by path and answer Slack-shaped JSON from
`tests/fixtures/` through the fake `connectors`; ids there are synthetic.

`b10x-loom plugin run` settles both models before any read: with `--catalog` from route aliases
(`model_catalog`), otherwise Codex models, as `run` does. A turn runs on the agent's llm `Model`
itself (`b10x_loom_cli::plugin::turn_models`): the chooser path sends each selection and argument
request to it, with nothing between and no other target. Without `--once`, the first SIGTERM or SIGINT sets the host's stop, so
a systemd stop ends it after the item being handled; a second one exits at once with 130
(`plugin_run_stops_on_sigterm`). `plugin report` reads `record.jsonl` without the lock and drops a torn
last line. `crates/loom-cli/tests/plugin.rs` runs the binary against the fake `connectors` and the
fixture catalog at a loopback socket replaying recorded responses.

## Laya selector

`b10x-loom-selector-laya` (`crates/loom-selector-laya`) is an experimental `FastTyped`
`ActionSelector`, never the default. It sends the goal and the candidate action ids as one
`choice` question to a Laya endpoint (`POST <base>/v1/systemone`, the wire of the Laya README at
commit `1adc59f`) through llm's `b10x-llm-http` `HttpClient::post_json`, on a runtime the selector
owns, and reports `answer_confidence` as the confidence, never `confidence`. A choice outside the
candidates, a non-2xx status, a timeout, malformed or over-deep JSON, or a probability outside
[0, 1] is `SelectorError::Unavailable`; more than 100 candidates is refused before sending. The
wire stays in this crate until Laya becomes a default selector, when it moves to llm. No product
crate (`b10x-loom-cli`, `b10x-loom-sdk`) depends on it, so Loom builds and runs with no Laya code;
`crates/loom-selector-laya/tests/laya_selector.rs` holds both, the dependency rule over the
resolved graph `cargo metadata` reports.

## Session executor

`b10x-loom-session-executor` (`crates/loom-session-executor`, `loom_sdk::session`) is Commission's
`AgentExecutor` over one metaharness session per call (Atlas ADR 0098): Harness, Claude Code or
Codex, in a scratch copy of the workspace's named files. It proposes and never performs: the
change returns as a `ProposedAction` whose arguments are `{files}` only, read against the bytes
the copy was made from, and an effect applies it after Commission revalidates it.
`SessionExecutor::from_spec` builds it from a `loom.agent/1` spec (kind, brief, `agent` role
model, `ToolPolicy`), launches Claude Code offering only `allow` less `deny`, hands the policy's
decision mode to metaharness, denies an offered tool it cannot check, and records the spec's
`agent.id` on each session; it applies no budget, stop policy or egress
(`crates/loom-session-executor/tests/from_spec.rs`). It accepts Claude Code and Codex only under
`decisions: ask` and b10x-harness only under `observe` with no `allow` or `deny`, and refuses
any other spec, or a tool name holding a comma or whitespace, with a `SpecError` before any
session. A file the
session changed that the workspace no longer holds as copied when the session ends makes the step
suspend with `ExternalAvailability` instead; a write after that is the effect's guard to catch.
A path with a `.git` part, or one reached through a link in the workspace, is neither copied nor
proposed; a named path a link reaches at the session's end and did not at copy time suspends the
step the same way, and is never read as gone. A named file spelled other than as one plain relative
path (a `.`, `..` or empty part, a root) suspends the step before any copy. `SessionExecutor::continuing(&row)` makes the next session it records name that earlier session as `continues` (lineage only: the request to metaharness is a fresh session's and no harness session is resumed); a row of another commission than the call's (compared as UUIDs), or a session id that is the predecessor's or one the executor already recorded, suspends the step with `ExternalAvailability` before any copy, naming the session and both commissions or the id (`crates/loom-session-executor/tests/lineage.rs`). It records each
session through the generated `loom.session` behaviour (`ess/domains/session.yaml`), a new
`SessionId` per call; the summary and the SHA-256 transcript digest travel on `ExecutorStep`,
never as evidence; a failed session suspends with `ExternalAvailability` and is no record. Each
call's `SessionReport::trace` (`SessionTrace`, read through `last_report`) is the session's
record projected to `trace-ir/1` and summarized by `metaharness::protocol::summarize`: the vendor
record metaharness kept, read again by its kind's reader (`project_vendor_record`), else the
`metaharness.event/1` lines the executor received; its digest is the step's. A record that does
not project keeps its digest and source, says why and changes nothing about the call; a session
that never started has no trace. `SessionReport::tool_decisions` (`loom.session.ToolDecision`)
lists each tool call's decision, its tool (none for an empty name), a denial's reason and who
decided it (`loom.session.DecidedBy`), in record order, read by `tool_decisions` from the
`trace-ir/1` document of the `metaharness.event/1` lines, never from a vendor record; they are
known only when that document carries metaharness's decision census, every requested call was
decided, and the decisions agree with the census in every count (allowed, denied, replaced,
abstained, and `by_decider`), else none (unknown), never an empty list: a b10x-harness
session's observed, undecided calls are unknown. They travel on the report only: never evidence, never in a
proposal, and not on `ExecutorStep`, whose `transcript_digest` names the record they were read
from.
A b10x-harness session reads its endpoint's API key from the operator's secret store when the host
names it (`SessionExecutor::api_key_secret`, `<namespace>/<name>`): the loop runs under `ApiKey`
with metaharness's `RunSpec.api_key_secret`, so b10x-harness gets `--api-key-secret` and opens the
store itself, and the key never enters Loom, metaharness or the child's environment. The endpoint
then names no subscription token (`token_file` empty, `token_pointer` none); a name beside one, on
a Claude Code or Codex session, or not of that shape (`metaharness_b10x::validate_api_key_secret`,
or holding a control character) suspends the step with `ExternalAvailability` before any copy
(`crates/loom-session-executor/tests/endpoint_secret.rs`).
A b10x-harness session streams its reply in `text-delta` fragments: the executor joins a
turn's fragments with no separator, keeping the line breaks they carry, and starts each turn
(`TurnStarted`) on a new line before it looks for the action line; a Claude Code or Codex text
event is a whole message, one line apart from the next
(`crates/loom-session-executor/tests/b10x_text_deltas.rs`). A retried turn (the loop's `turn-retried`,
which metaharness passes on as an opaque event) drops what its failed attempt streamed
(`crates/loom-session-executor/tests/adversary_w20261024w2_u1.rs`).
It is the one crate that links metaharness (`metaharness` and
`metaharness-b10x` at tag `0.15.0`; move them together); `b10x-loom-sdk` and
`b10x-loom-conformance` reach it through it. No `metaharness` program runs. Its tests run
scripted sessions only (metaharness's `ScriptedRunner`): no harness program, no model, no
network.

## Workflow definitions

`b10x_loom_executor::workflow` (`story:dw-workflow-definition-executor`) runs a Loom workflow
definition, the `loom.workflow` domain (`ess/domains/workflow.yaml`), as Commission's
`AgentExecutor`. A definition names its protocol by catalog reference and orders its steps: a
deterministic step (one `action`, literal `arguments`) is proposed without a model; a judgement
step runs on its executor narrowed to its `actions` that the host's own narrowing
(`WorkflowExecutor::with_admitted`) admits, never widening it; `when` makes a step wait on a
frontier claim, never on a model.

A judgement step's executor (`story:judgement-step-any-executor`) is the host's own
`AgentExecutor` when the host supplies one (`WorkflowExecutor::with_executor`, which builds it for
each call from a `workflow::Judgement`: the workflow, the step, its `instructions` with the data
block of its inputs, and the actions it may propose), and otherwise Loom's chooser over the llm
`Model` the workflow is given (`WorkflowExecutor::new(governor, model, …)`: a `Loom` over
`model_selector`'s `ModelSelector` and `ModelArguments`, revalidating against the governor, one
forced `select_action` request among the step's actions, then one forced `action_arguments`
request, each led by the step's instructions). Either is handed the frontier listing no action
but those. A proposal of another action, or of one the frontier does not list as `Admissible` or
`ApprovalRequired`, is never returned, nor are reads proposed together (`ProposedReads`). What the
host's executor reports a call spent (`run_with_usage`) is the workflow call's usage, and the run
record written at the end of that call already carries it in the call's step, as the
`MeteredExecutor` counts it once the call returned, so a run resumed from that record reports it;
a later record or `save()` takes it from the meter, once. The chooser reports none, its requests
being counted at the metered model. A chooser answer without a usable
call is no useful action, bounded by Commission's idle rule; a request never answered suspends the
run for an external availability. Given `with_max_input_tokens`, a chooser call whose requests'
input tokens reach it, or report none as final, suspends the run for it (`LoopBudget`,
`max-input-tokens` or `budget-unobservable`, the two causes `loom.run.LoopStop` declares), and the
answer that reached it is not taken. The judgement step runs no model loop.
`WorkflowExecutor` takes the first step after the last performed one whose `when` holds; a step
whose actions the frontier does not list as `Admissible` or `ApprovalRequired` ends the call
`NeedsHumanJudgment`, naming the step and the frontier's reasons, and nothing later is taken
instead. `RecordingEffects` wraps the host's effect port into an `EffectLog` the executor reads: a
step advances only once the request it proposed was `Performed`, and a refused or stale one is
proposed again, bounded by Commission's idle rule; a refusal the port marks final ends the run. Commission revalidates every proposal, a
deterministic one included. `workflow::load` reads YAML into the generated types and checks it
against the protocol's actions, which the host supplies (`loom_protocols::ProtocolEntry::actions`,
so the executor does not depend on `loom-protocols`): every action is its protocol's, step names
are unique, with no surrounding whitespace, and a judgement step names an action; a refusal names
the step. Built `.with_meter(&meter)` under a `MeteredExecutor` on the same `RunMeter`, it labels
each metered step (`loom.metering.StepUsage.label`) with the workflow step the call took. The
position is kept in process (`decision-blocker:dw-run-state-across-restart`).

A step declares typed `outputs` (`story:dw-typed-step-outputs`; `loom.workflow.StepOutput`: a
`name`, a JSON `pointer` into its `Performed` report and a `loom.workflow.OutputType`), and a later
deterministic step's `Json` arguments name one at any position as `{"$step": <step>, "$output":
<name>}`; ESS types the declaration, and the reference stays inside the `Json` arguments. When
`RecordingEffects` records a step's request `Performed`, each output is taken from that report
and its type checked (a run the governor completes right after it still has them), kept
in process (`WorkflowExecutor::outputs`); a reference is resolved when its step is proposed,
and the resolved arguments are the proposal's, which Commission revalidates like any other. No
model supplies an output. An output absent from the report (a member named twice included) or of
another type ends that call and every later one `NeedsHumanJudgment`, naming the step and the
output (`output`), and nothing more is proposed; a reference to an output its step did not produce
in the run (the step was skipped) does the same naming the referring step. `load` refuses,
naming the referring step, a reference to a later step, to itself, to an unknown step or to an
undeclared output, and a malformed reference; and, naming the declaring step, an output declared
twice or a malformed pointer.

A judgement step lists the earlier outputs it is given (`story:dw-judgement-step-inputs`;
`inputs`, a list of `loom.workflow.OutputReference`: a `step` and an `output`). When the step is
proposed, the runtime appends exactly those values to the step's instructions as one data block,
`<workflow-inputs>` around a JSON list of `{step, output, type, value}` with the type its step
declares, introduced as data and never as instructions, and the step's executor is told those
instructions (`Judgement::instructions`); `<`, `>` and `&` inside it are written as `\u` escapes,
so no value can close the block. The model never reads them from the transcript, and no model
supplies an output. A listed output the run did not produce (its step was skipped) or one not of
its declared type ends the call `NeedsHumanJudgment` naming the judgement step and the output, and
calls no executor. `load` refuses, naming the judgement step, an input naming a
later step, the step itself, an unknown step or an undeclared output, and an input listed twice.

## Intake and the command line

The CLI's `Briefing` owns single-intent working context. `--context-policy bounded` is opt-in;
`run` and existing `SliceRequest` callers retain legacy behavior. `run_with_options` also accepts
a measurement-report path. Preserve typed revision-bound test state, exact intent and instructions,
complete history records, and the 64 KiB serialized request ceiling including schemas and lookups.
`crates/loom-intake-slice/tests/bounded_context.rs` compares recorded workflows in both policies.
This does not change the separate governed-loop compactor. Reports contain counters only and retain
unknown provider counters as unknown; the catalog-based CLI also reports routing and startup failures.

Result capture and selection belong to `loom-intake-slice::results` and the shared `Briefing`.
Resolve references before returning a `ProposedAction`, never inside an already-admitted effect.
Keep original statuses and capture completeness explicit; stored text is data, not authority or
verified evidence. The store expires with its briefing and grants no cross-run access. The
`result_reference_workflow` integration suite holds the inspect-to-edit path and bounded context.

- `b10x-loom-intake-router` classifies an intent against the host catalog (legacy wrappers use engineering definitions) and
  refuses a pick outside it or below the confidence threshold. A routing proposal is never
  authority; whoever opens the case checks it.
- `b10x-loom-intake-references` extracts tracker keys, chat permalinks, merge and pull requests and
  URLs from an intent, deterministically.
- `b10x-loom-intake-slice` is the local effect adapter (`LocalEffects`) and a thin caller of
  `run_until_blocked`. It has no loop of its own; keep it small. It executes `software.change/1`
  actions inside the given workspace and `system.query/1` through the host clock, never merges, pushes or deploys, and never supplies
  authority on the operator's behalf. It submits evidence only from what it did itself (§ the
  slice's evidence below). It performs `tests.write` and `implementation.edit` (each a committed write, reporting the new
  `HEAD`), `repository.inspect` and `tests.run`; a resume hands off `repository.merge`. On a
  `software-change@1` case the runtime reads every frontier without the actions the slice can
  never perform (`run::never_performed`: gated and not in `run::PERFORMED`, which is
  `release.rollback`), so such an action is never offered, awaited or held as a Suspension, and a
  proposal of it is refused as not in the frontier; `crates/loom-cli/tests/slice_run.rs`
  (`the_run_never_offers_or_awaits_release_rollback`) holds it. `run::SliceReads` filters only a
  `software-change@1` case's frontier and passes any other protocol's through unchanged.
- `tests.write` writes only test paths: the slice performs it only when every path it changes
  matches one of the host's test-path patterns (`test_paths::TestPaths`), and otherwise refuses it
  naming the first path outside them before anything is written, for every change kind, so a
  bugfix cannot change its implementation through `tests.write` before a red test;
  `implementation.edit` is not restricted. The patterns are trusted context: `run --test-path
  <GLOB>`, repeatable, replacing the defaults (`tests/**`, `**/tests/**`, `test/**`,
  `**/test/**`, `**/*_test.*`, `**/test_*.*`, `**/*.test.*`, `**/*.spec.*`), or
  `SliceRequest::test_paths`; the resume context keeps them (`intake.routing.SliceRun.test_paths`,
  `ResumeContext.test_paths`). `crates/loom-intake-slice/tests/selector_executor.rs`
  (`tests_write_writes_only_the_hosts_test_paths`) holds it. A resume's request carries the held
  case's inputs, read from the governor (`intent::resumed_request`).
- `run --risk <trivial|standard|elevated|critical>` (default `standard`), `--change-kind
  <feature|bugfix|refactor>` (default `feature`) and `--affects-security-boundary <true|false>`
  (default `false`) are the case inputs (`intake.routing.CaseInputs`, `case::RUN_DEFAULTS`),
  given to every protocol that declares them and to no other; no model, classifier or router
  sets one. With the defaults a run reaches the outcomes it reached before the inputs; with
  `--change-kind bugfix` an `implementation.edit` is admissible only once a test failed on the
  current revision (`a_bugfix_edits_the_implementation_only_after_a_failing_test`).
- The slice's evidence is `intake.routing.SliceEvidence`, submitted only by the verifier
  (`verifier::TestResultVerifier`) from what the executor did: a `test_result` for every
  `tests.run` on a committed revision; a `regression_test_result` only from the host's
  regression-test command (`run --regression-test-command <CMD>`, `{path}` in a word;
  `executor::RegressionTestCommand`, `TestCommand::with_regression`, kept in the resume context
  as `intake.routing.ResumeContext.regression_test_command`), which on a `bugfix` case, after a
  `tests.run` that follows a `tests.write` that committed (the executor's own history, not
  carried across a resume), the executor runs once per test path those writes committed, each
  its own observation (`TestRun::regressions`, `executor::REGRESSION_RUN_SOURCE`), never from
  the suite; without the command none, so `red_first.required` stays open
  (`crates/loom-intake-slice/tests/regression_command.rs`); a path whose bytes at the run equal
  its bytes before the run's first write to it (written with what it held, or written and then
  written back) is no committed test path; and an
  `implementation_change` about the new `HEAD` for every `implementation.edit` that committed.
  Each record carries `observed_at`, the host clock's instant of its observation
  (`LocalExecutor::with_clock`, `run::run_with_clock`; `clock::SystemClock` by default), and the
  verifier's producer; the governor gives Canon `EvidenceData.producer`, never a producer in the
  facts (it removes a facts `producer`, and the host's `target`, before Canon reads the record),
  and reads no clock. It
  takes a record's `observed_at` only as the instant of the observation the record cites and
  that it received: a record citing an observation it never received, with or without an
  `observed_at`, or with any `observed_at` member other than that instant, is refused and nothing
  of it is kept, and
  `CanonGovernor::check_evidence` names the record (`EvidenceRefusal`;
  `crates/loom-governor/tests/evidence_instant.rs`). A
  bugfix that writes its test, runs it red and then edits discharges `red_first.required`, and one
  that edits before any regression test leaves it open. Canon orders records in whole seconds,
  so the executor keeps its observations' instants strictly increasing in whole seconds: it
  waits on the clock (`Clock::wait`, at most `executor::CLOCK_WAIT`, 2 s) for the previous
  observation's second to pass, and a clock that still has not (one that stepped back) fails the
  step with `ExecuteError::Clock`, naming both instants, before the observation is delivered or a
  record submitted; an edit already committed stays committed. Tests set the instants with an
  injected clock that advances when waited on
  (`crates/loom-intake-slice/tests/adversary_w10_u1.rs`). The governor `b10x-loom` builds is
  `run::slice_governor`, generic over `T: EvaluationTime` through the slice, evaluating at the
  host instant (`clock::HostTime`), so an evidence kind's `max_age` expires an old observation.
  `crates/loom-cli/tests/slice_run.rs` holds it
  (`a_bugfix_red_regression_test_before_its_edit_discharges_red_first`,
  `a_slice_governor_expires_an_operational_observation_after_max_age`).
- `b10x-loom-cli` is `b10x-loom`. Its clap definition is the library (`src/lib.rs`, `Cli`), which
  `loom-docs` renders. The library also holds `evaluate`, the JSON codec of `b10x-loom evaluate`
  over the generated `loom.evaluation` types, and `exit_status` and `events`, the writer of
  `run --output jsonl`: one `intake.events` record per line (`ess/intake/domains/events.yaml`),
  built from the generated types, the terminal record last and exactly once.
  `crates/loom-cli/tests/run_events.rs` holds it; `loom-docs` checks the hand-written
  `website/docs/reference/run-events.md` against the declaration, the fields of a record a kind's
  field holds included. `EventStream::sessions` wraps a `SessionExecutor` so each agent session
  writes one `Session` record from its `SessionTrace`, and none of its turns writes a `Turn`,
  `ToolCall` or `Usage` record; `b10x-loom run` itself runs no session executor
  (`crates/loom-cli/tests/session_events.rs`).
- `run --state <DIR>` puts the case in a `FileCaseStore` and the commission's Suspension in a
  `FileSuspensionStore` over `<DIR>`, keeps the commission's resume context
  (`intake.routing.ResumeContext`, `b10x_loom_intake_slice::resume::ResumeContextStore`, under
  the same lock and atomic write; once per commission, as `intake.routing.KeepResumeContext`
  declares) before the commission runs, and runs it through
  `run_with_suspension` (`run_prepared_intent_with_state`). A run that ends without the
  Suspension held removes its context (`intake.routing.DropResumeContext`); a process that dies
  first leaves one, so a reader pairs each context with its commission's Suspension. Before any
  model call, `ResumeHost::check` refuses a path the context cannot keep as text, and each store's
  open proves the directory takes a write. With `--config`, the context keeps the
  configuration's absolute path (`ResumeContext.config`) and, in `model`, the name serving the
  role `agent`; `resume` loads it again (`loom_sdk::models::from_config`), resolves that name and
  checks its credential before any model call, refusing exit 1 with the file and the role. A
  context naming both `config` and `catalog` is refused when it is kept and when it is read.
  Without it `run` keeps the memory
  store and `run_until_blocked`. `crates/loom-cli/tests/run_state.rs` runs the binary to a held
  Suspension and reads `<DIR>` back through the stores.
- `approve <COMMISSION> <CAPABILITY> --state <DIR>` writes a grant (`intake.routing.Grant`,
  `b10x_loom_intake_slice::grant::GrantStore`) for the request the commission's Suspension holds
  and performs nothing. `resume <COMMISSION> --state <DIR>` reads the context only for a
  commission holding a Suspension, rebuilds the run from it (`resume_with_state`), and continues
  it through `resume_after_grant` with `GrantAuthority`, which allows only the request the
  Suspension holds while it is `Pending`. `resume` reports the workspace's `HEAD` to the case
  first (`case::report_head`), so a commit made since the Suspension is refused `CaseMoved`. A
  granted `repository.merge` is bound to `handoff::HandoffEffects`, which records a merge
  hand-off (`intake.routing.MergeHandoff`, once per request and once per `HEAD` of a
  commission, only at the implementation revision the case holds) and moves no Git ref; only a
  resume binds it. A resume runs on `handoff::SettledHandoffs`, which settles a merge the agent
  proposes again at a `HEAD` the commission already handed off (`HoldSuspension` `settled`), so
  it holds no Suspension and leaves no resume context; `approve` still refuses a merge at such a
  `HEAD` (`AlreadyHandedOff`) as a defence. With a state directory, a stop `ApprovalRequired`
  reads as approvable only when the commission holds a Suspension (`run::ApprovalGate`):
  otherwise it prints
  `..., nothing suspended` (exit 4) or, in a resume at a handed-off `HEAD`,
  `stopped: MergeHandedOff (<request>, HEAD <sha>)`. Each `ResumeRefusal` and `ResumeError` has
  its own exit status (`resume_exit_status`, `RESUME_HELP`).
  `crates/loom-cli/tests/resume_after_grant.rs` runs `run`, `approve` and `resume` as separate
  processes.
- `run --action-bindings <FILE>` reads a run configuration (`intake.bindings.ActionBindings`,
  `ess/intake/domains/bindings.yaml`; `b10x_loom_intake_slice::bindings`): one installed
  protocol, declaring the `intent` artifact alone and no case input, and one argv per action.
  `bindings::check` refuses, before any model call, a bundled protocol, an action the protocol
  does not declare, one bound twice and an empty argv. A pick of that protocol with an action left
  unbound stops `NoLocalExecutor` naming it, before a case opens, its `SliceRun::unperformed`
  (the jsonl terminal record's `unperformed_actions`) every unbound action; otherwise
  `bindings::CommandEffects` performs each admitted action by starting its program directly (no
  shell) with the arguments as JSON on stdin, only `PATH` in its environment and a timeout past
  which it and the processes it started are killed and reaped: exit 0 is `Performed` with its
  stdout (one JSON object) as the report, anything else `Refused`, a status that cannot be read
  included, naming the error; a timeout this host's clock cannot hold is `Refused`, naming it,
  before the program starts (`crates/loom-intake-slice/tests/bound_command.rs`,
  `a_timed_out_command_leaves_no_process_it_started`,
  `a_timeout_too_large_for_the_clock_is_refused_and_starts_no_command`). Evidence comes only from the report's `evidence` list
  (`intake.bindings.CommandEvidence`), each of a kind the action declares it may produce, about
  `intent`; another kind fails the run. A binding confers no authority: the slice's
  authority provider still answers every capability with approval required, and `resume`
  continues `software-change@1` runs only. Substrate does not confine a bound command, so
  without `--unconfined-bindings` `run` stops `ConfinementUnavailable (CommandUnconfined: ...)`,
  exit 3, before any model call (`bindings_refused`); with it, standard error says so. `crates/loom-cli/tests/custom_protocol_commands.rs`
  (`a_custom_protocol_runs_through_declared_commands`) runs the binary against scripts.
- With `--mandate <FILE>`, `approve` and `resume` clear the approval through Mandate (§ Mandate);
  a Mandate refusal is `NotGranted` (16) and performs nothing. A `resume` without `--mandate`
  refuses a grant naming a Mandate approval `NotGranted` (16) and performs nothing.

The injected `TestRunner` defaults to Substrate 0.10.0, with host and wire pinned together.
Tests get no network, a cleared environment, read-only source and Rust toolchain, writes only
under `target/`, and resource bounds. Mount dependency-cache subdirectories only; never Cargo's
credential/config files or the operator's home. Dependencies are prefetched explicitly by the
operator; the private Cargo home is offline. A missing guarantee stops with a typed refusal;
never silently switch to `UnconfinedRunner`. Test fixtures opt out explicitly.
Validate writable artifacts before every launch: external hardlink aliases and special files
are invalid scopes; internal Cargo hardlinks remain supported.
The CLI attempts one delegated user systemd scope and guards against re-exec loops.
Every executed test observation includes the actual applied confinement; refused launches produce
no test observation or passing evidence. Real delegated tests must run to qualify confinement;
a skip with a named missing prerequisite establishes no qualification.
`SubstrateRunner::run_child` confines a child process the host names (`ChildCommand`: program and
arguments, the directory adopted as the workspace, the workspace directories it may write beside
`target/` (`with_writable`), and Substrate's `ExecEnvironment`,
`SandboxProfile` and `NetworkMode`, `NetworkMode::None` by default, or with `with_egress`, once
per aperture, the Substrate `EgressAperture`s it may reach, at most `MAX_EGRESS_APERTURES` (4),
and with `with_ca_bundle` the certificate bundle it
trusts there) on the same launch path:
writes only under `target/` and the directories named (Substrate's `WorkspaceAccess::Scoped`;
`.git` only when named; a path that is absolute, climbs out with `..`, passes a symlink, is absent
or no directory, holds a special file or a hard link from outside, or overlaps another is refused
`ScopeInvalid` before launch; `TestRunner::run` keeps `target/` alone, and the applied
confinement's `writable_scopes` lists them, a record naming another set refused), no toolchain or
cache mounted, exactly the named environment (with a bundle, plus `SSL_CERT_FILE`), a typed
refusal for a missing guarantee, only the requested network accepted as applied, no network or
exactly the named apertures, in order (name, pinned destination, mechanism, ceiling, and for
several the address `127.0.0.<n>` each listens on), checked after the child
ran and refusing its result as a test's is refused, a result with no exit status refused unless
its timeout or its aperture's byte ceiling (`exec.aperture-byte-limit`, named in its output, its
driver record counting the bytes) ended it, and the applied confinement in its `TestExecution`,
whose `network` lists every aperture (`intake.confinement.AppliedAperture`: name, destination
and, for several, `listen`), a record naming another set, order or address refused; an empty
list is no network, and `UnconfinedRunner` leaves it absent (`None`: no network confinement
applied), never read as no network (`security_an_unconfined_run_does_not_report_no_network`).
The child reaches each aperture by its host's name, which Substrate maps to its forwarder inside
the sandbox: `127.0.0.1` for one aperture, `127.0.0.<n>` for the n-th of several. An
aperture declaration Substrate's daemon never makes (no host or port, a name it refuses, a zero
byte ceiling, a host that is no host name or numeric host, a pinned address that is no IPv4 one or
not at the named port), and a numeric host other than `127.0.0.1` spelled exactly so, which the
child could not reach the host by, are refused `ScopeInvalid` before anything starts, the refusal
naming the host. A numeric host is one `inet_aton` reads as an address (`numeric_host`): one to
four dot-separated labels, a trailing `.` ignored, each a decimal, `0x`/`0X` hexadecimal or
leading-`0` octal number, so `0x7f000001`, `0177.0.0.1` and `127.1` are numeric, because glibc
dials such a host before it reads the child's `/etc/hosts`. One Substrate did not publish when
the driver opened (its mechanism unproven) is refused
`CapabilityUnserved`. Several apertures are refused `ScopeInvalid` before anything starts when
there are more than four (naming the count and every aperture with its host), when one name is
given twice (naming it and both its hosts), when two name one host (ASCII case aside, a trailing
`.` ignored; Substrate maps a name to one aperture; naming both apertures and the host) and when
the second to fourth names a numeric host or `localhost`, which only the first listens on (naming
the aperture and the host). A certificate bundle
(`with_ca_bundle`) is read once per launch through an open with its last component not followed
(`O_NOFOLLOW`), and the child trusts the bytes read then: the driver is given a private
read-only copy of them in its own state directory (`CaBundle::write_private_copy`, removed with
the driver) as Substrate's `HostConfig::ca_bundle`, never the named path, and Substrate binds a
snapshot of that copy at `/etc/ssl/certs/ca-certificates.crt`, which `SSL_CERT_FILE` names;
Substrate's record names no bundle, so it is not checked after the run. The bundle is the trust
store the child gets through `SSL_CERT_FILE`; beside it, the environment may not name the listed
variables, so a host does not override by accident the bundle it named. That is no guarantee
about what the child trusts: configuration under `HOME`, a program's own flags and variables not
listed can still point a client elsewhere or turn verification off; what the child reaches is
bounded by the apertures. A bundle for a child with no aperture, a path that is not UTF-8 or not
absolute, one beside an environment naming a listed variable (`TRUST_STORE_VARIABLES`:
`SSL_CERT_FILE`, `SSL_CERT_DIR`, `CURL_CA_BUNDLE`, `CURL_CA_PATH`, `CURL_HOME`,
`REQUESTS_CA_BUNDLE`, `NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO`, `GIT_SSL_CAPATH`,
`GIT_SSL_NO_VERIFY`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_PARAMETERS`; or a
`GIT_CONFIG_KEY_<n>` naming `http.sslCAInfo`, `http.sslCAPath` or `http.sslVerify`, any case, URL
subsection or not, `GIT_TRUST_KEYS`; each named in the refusal; Git configuration naming other
keys stays allowed), and one that does not exist, is a symbolic link, is no regular file, is
empty, holds more than 1 MiB (`MAX_CA_BUNDLE_BYTES`, Substrate's ceiling) or cannot be read,
on the path and again on the file opened, are refused `ScopeInvalid` before anything starts,
naming the path (`check_ca_bundle`). A program containing `=` or starting with `-` (test or child)
is refused before anything starts: Substrate execs through `/usr/bin/env`, which reads it as an
assignment or an option. A runner and its clones share one driver per workspace directory, set
of egress apertures (the whole set, in order) and certificate bundle, the bundle compared as its
path and the bytes read
(`DriverEgress`: Substrate fixes a driver's apertures and bundle when it opens), so children naming
different bundles, or a bundle whose bytes changed, never share one,
because opening a driver kills every `substrate-ex_*` cgroup under its root; each launch discards
its finished execution from the driver (`discard_superseded_exec`), which tracks at most 128; no
driver opens on a root while a launch of this process runs there (the process-wide count in
`confinement.rs`, keyed by the canonical root), so the launch needing it is refused instead.
A `TestExecution` carries the child's two streams apart (`child_output`, `intake.confinement.ChildOutput`),
each whole up to Substrate's output limit and flagged cut past it; `output` stays the 8 KiB
merged tail. `ChildCommand::with_stdin` (at most `MAX_INPUT_BYTES`, 16 MiB, refused before launch)
runs the child as a Substrate raw-pipe session, writes the bytes once in 64 KiB frames and closes
its input. `SubstrateRunner::spawn_child` (`story:live-confined-child`) makes every check
`run_child` makes before launch and returns a `ConfinedChild` (`confinement/child.rs`) running
in the same kind of session: the host writes and closes its input, reads `ChildFrame`s naming
their stream (`intake.confinement.ChildStream`), signals it (`ChildSignal`: an interrupt or
termination is followed by a kill 5 s later) and waits for it; `wait` drains its output and
settles the result as `run_child` does, the applied confinement checked against the request, and
a missing exit status is a result only when its timeout or a signal the host sent explains it.
A pipe session runs only on a driver whose capability facts publish the namespaces, cgroup
limits, cgroup kill, the three signals, no egress and an output limit (`check_pipe_sessions`),
else `CapabilityUnserved`; a command with `with_stdin` handed to `spawn_child` is refused. A live
child holds its launch in the process-wide count and a multi-thread runtime of its own, so
Substrate keeps supervising it between calls (call its methods outside an async runtime); dropped
before it ended, inside an async runtime or not, it is killed, and either way its execution is discarded from the driver.
A child reads host paths read-only only where the host names them
(`story:child-read-only-roots-and-host-paths`): `with_read_only_root(host, mount)` a directory,
`with_read_only_file(host, mount)` one file, `at_host_path(host)` either at its own path, decided
on disk at launch. `check_mounts` refuses `ScopeInvalid`, before `target/` is created and naming
the path, a host path or mount that is not UTF-8, absolute and canonical (Substrate's wire text),
a host path that is absent, reached through a link in any component or of the wrong kind, a mount
on, under or containing one Substrate owns (`RESERVED_MOUNTS`, `/runtime`, `/scratch`,
`/etc/hosts`, the bundle path), two mounts equal or nested (component-wise), more than 4 roots or
16 files, a host path that holds the bundle the child names or the temporary directory the
driver's state is made in, and one that is, holds or lies inside the workspace; then Substrate's
own `validate_read_only_roots` and `validate_read_only_files`. Once the driver opened,
`check_driver_mounts` refuses a mount reaching its state directory and files on a driver without
`exec.read-only-files` (`CapabilityUnserved`). The record must name exactly the roots and files
requested, for `run_child` and a live child alike (`check_record`), and `AppliedConfinement`
projects both; a record naming another working directory than the one requested (none when none
was), a writable root, which Loom never requests, or several apertures where it asked for one or none, is refused (`a_record_reporting_a_workdir_writable_roots_or_apertures_is_refused`),
as is one naming an execution capsule or its filesystem, or a single aperture naming a listen
address (`security_a_record_naming_an_unrequested_capsule_is_refused`,
`security_a_single_aperture_record_naming_a_listen_address_is_refused`); `check_record` takes
the record apart without `..`, so a member a later Substrate adds does not compile until judged.
What remains after the check: Substrate 0.10.0's admission opens each root and file once,
refusing a link at any component (`openat2` with `RESOLVE_NO_SYMLINKS`), and bubblewrap binds
the descriptor it opened (`--ro-bind-fd`; Substrate's `crates/substrate-host/src/read_only_roots.rs`
and `read_only_files.rs`), so only a directory or regular file put in its place between Loom's
check and that admission is bound; nothing swapped after admission is. A root is read-only to the
child alone: one under a directory another confined child may write can change while it is read.
`ChildCommand::sandbox_path` maps a host path to the workspace (`/workspace/...`), a root's mount
or a file's mount, never a path with `..`, through a link, or sharing only a text prefix. Substrate
0.10.0 offers writable roots; Loom requests none (`story:writable-host-root`), so a child writes
only in its workspace. A host may declare all of this as one value, `intake.confinement.ConfinementSpec`
(`story:confinement-spec`): `ChildCommand::confined(&spec)` (`confinement/spec.rs`) applies each
field through its builder, so the launch checks it as it checks the builder calls, refuses a
non-empty `writable_roots` `CapabilityUnserved` naming it, and refuses `ScopeInvalid`, naming the
field, a value the builder's type cannot carry (`crates/loom-intake-slice/tests/confinement_spec.rs`).
A child starts in `/workspace` unless the host names another directory
(`story:child-workdir`): `ChildCommand::with_workdir` takes the path as the child sees it, the
text Substrate's wire and record carry (a host holding a host path names it through
`sandbox_path`). `check_workdir` refuses `ScopeInvalid` before `target/` is created, naming it, a
working directory that is not UTF-8, longer than `MAX_WORKDIR_BYTES` (4096, Substrate's), not
absolute, holding a control character (named escaped) or a `.`, `..` or empty component (a
trailing `/` included), at or beneath
a read-only file, or not at or beneath `/workspace` or a read-only root's mount (`/scratch`,
`/runtime` and `/`, which Substrate would admit, are not placed by Loom); then it walks the host
disk from the canonical workspace or the root's host path and refuses one that is absent, no
directory or reached through a link, except `/workspace/target`, which the launch creates, and
one this process cannot search, or below a directory it cannot search (`may_search`), which
bubblewrap could not enter. Substrate checks it again under the descriptor it bound; a run whose
output still carries bubblewrap's `Can't chdir to <workdir>` diagnostic is refused `ScopeInvalid`
once it ended (`check_entered_workdir`, in `settle`, so in `run_child` and `ConfinedChild::wait`),
so a program that never ran reports no exit status. What remains: a directory removed or swapped
for a link between Loom's check and the launch is refused by Substrate's walk, but that refusal
reaches the host as `CapabilityUnserved`, not `ScopeInvalid`. Once the driver opened,
`check_workdir_served` refuses a working directory on a driver without `exec.workdir`
(`CapabilityUnserved`). The profile digest keys a working directory only when one is named, so a
child without one keeps the wire, record and digest it had.
`crates/loom-intake-slice/tests/confined_child.rs` holds it; its delegated
cases start their own test binary under `systemd-run --user --scope -p Delegate=yes` and skip
naming the missing prerequisite where bubblewrap, a user systemd manager or the driver is absent,
the egress cases where `bash` or Substrate's aperture is, and the trust-anchor case where
`openssl` or `curl` is; their hosts are loopback listeners, the TLS one signed by a CA the test
makes.
The slice's own git calls
go through one helper, `crates/loom-intake-slice/src/git.rs`: no workspace hook, `core.fsmonitor`
command or signing program runs, a case does not open on a workspace whose own configuration
names a program, and a call is refused once that configuration, its includes or the git directory
changed since the case opened. `tests/host_git_hardening.rs::every_host_git_command_goes_through_a_hardened_helper` rejects
other Git call sites except the separately hardened pinned-source helper at
`crates/loom-protocols/src/git.rs`. That helper fetches into a private bare repository, never a
run workspace, with hooks, fsmonitor, helpers and global/system configuration disabled. Gates still run when the operator or the bot commits and pushes.

Model calls go through llm's crates at release tag `0.8.1`, never a hand-written HTTP client.
`model_retry.rs` owns caller retries using llm's retry
classification and `RetryPolicy`: three identical attempts, no retry after a sink event,
cancellation-aware backoff and per-attempt context metrics. A model's turn stays single-attempt;
effects remain outside this loop. `system_query.rs` exercises recovery in all three request phases. The Codex preset and the forced tool call are
`codex_model` (the CLI builds its model with it) and `call_tool` (the router calls it). With
`run --catalog`, the CLI builds both models instead with `b10x-llm-models` `port` from the
route aliases `--model` and `--classifier-model` name (`crates/loom-cli/src/model_catalog.rs`,
held by `crates/loom-cli/tests/catalog_route.rs`): one target per route, no fallback, and no
credential resolver, so a credentialed account is refused before any model call. The
credential is the operator's Codex login (`~/.codex/auth.json`), read and renewed by llm, never by
Loom code. Loom ships no model port and no provider wire of its own: an SDK host hands Loom its
llm `Model`, and a Claude Code session is metaharness's to run (ADR 0098).

## Planning and waves

- The plan is the AEP store under `.engineering/`, written only through `aep plan artifact`. Body
  drafts go in `.engineering/drafts/` (git-ignored). Draft stories are planned, never shipped.
- A wave runs on `wave/<date>-w<n>`: `plan: open wave … (story:…)`, one `impl/<story>` branch per
  story merged into the wave branch, then `plan: close wave …`, and `main` fast-forwards to the
  wave head. Work outside a wave lands as a bot pull request.
- Use a managed worktree (`worktree create --repo ~/beyond10x/loom --purpose …`); keep the primary
  checkout clean. Run `git fetch origin` first and pass `--base origin/main`: `--base main` takes
  the local `main`, which can be behind, and a branch cut from it conflicts or cannot fast-forward.

## Commits, pushes and GitHub writes

Every commit and push is `b10x-bot[bot]`'s through `b10x-gates bot`. Every other GitHub write
(pull request, comment, issue, release, workflow dispatch, re-run) goes through `b10x-gates api`.
`gh` is read-only here. Loom is enrolled in common Gates (`shared-gates.yml`); its
`common / Security and privacy` check scans every commit since the policy baseline, so a
`/home/<name>/` path literal in any commit fails it and admits no exception.
A pull request merges only once its head carries the required check `b10x-gates / local evidence`:
run `b10x-gates --repository beyond10x/loom check --receipt <file>`, then `publish --receipt
<file>`, in a tree at that head. The bot push scans every blob since the baseline, so a partial
clone (`blob:none`) needs `git backfill` first.

## Releases

Loom releases from source at bare-version tags (`0.1.0`, no `v`); every crate inherits the
workspace version and nothing is on a registry. Consumers pin `tag = "<version>"`.

`CHANGELOG.md` follows Keep a Changelog 1.1.0 and collects under `## [Unreleased]` between
releases. A change adds one bullet per change a consumer sees under `### Added`, `### Changed`,
`### Deprecated`, `### Removed`, `### Fixed` or `### Security` (in that order, each only when it
has a bullet): one line of about 120 characters naming the public item in backticks, its story id
in parentheses where it fits. A break for a consumer is a bullet that starts `**Breaking:**`, one
per break. No prose paragraph: the detail stays in the story and the commit. A path the SDK
stability page (`website/docs/reference/sdk-stability.md`) lists is removed only after at least
one release listed it, or its module, under `### Deprecated` (§ The promise of that page says what
reads as a removal). A release whose entry has a `**Breaking:**` bullet
moves the minor version before 1.0, never only the patch version. A release is:

1. A release commit on `main` (through a wave or a bot pull request): `[workspace.package] version`
   in `Cargo.toml`, `Cargo.lock`, and `## [Unreleased]` turned into `## [<version>] - <date>`
   under an empty `## [Unreleased]`, carrying its `**Breaking:**` bullets or, when nothing breaks,
   the line `No breaking changes.` under the heading; the link references at the bottom move with
   it (`[Unreleased]` compares from the new tag, and a `[<version>]` line links its tag). README,
   AGENTS.md and the site pages that name the version or a dependency line move with it. A
   release moves the `ess` pin (§ ESS) to the newest ESS release in that same release.
   `cargo run -q --locked -p loom-xtask -- release-check --tag <version>` passes on it.
2. `check` and `Shared source gates` green on that commit.
3. An annotated tag by `b10x-bot[bot]` on that commit: `b10x-gates bot … -- tag -a <version> -m
   "Loom <version>" <commit>`, then `-- push origin <version>`.
4. `release` (`.github/workflows/release.yml`) green on the tag. It runs `loom-xtask release-check`
   (the tag equals the workspace version and `CHANGELOG.md` has its entry, with a `**Breaking:**`
   bullet, or `No breaking changes.` only when no stable surface changed since the previous tag,
   so the job checks out the full history and its tags) and then `check.yml` on the tagged
   commit. It is read-only, needs no secret and creates no release.
5. The GitHub Release for the tag, created by the bot (`b10x-gates api --method POST --path
   /repos/beyond10x/loom/releases`), named `<version>`, with short notes built from the entry's
   bullets: one or two summary lines drawn from them, a **Breaking** list of its `**Breaking:**`
   bullets (or `No breaking changes.`) and a **Changed** list of its other bullets, one line per
   item and at most 8 each, and a link to the version's section of `CHANGELOG.md` at the tag. The
   entry keeps every bullet; every line of the notes is taken from it.

A release is finished when the tag, the green `release` run and the bot's GitHub Release with its
notes are verified. A pushed tag whose `release` run is not green is queued, not released; never
move or delete a pushed tag, fix forward with the next version.
`crates/loom-xtask/tests/release_check.rs` holds the check, the workflow's shape, and that the
README and site pin the workspace version as a tag.

## Documentation

The site (`website/`, Docusaurus on `@beyond10x/docs-system`) is written with the workspace `docs`
skill (`~/beyond10x/.agents/skills/docs/SKILL.md`). It is independent of the unified site:
`pages.yml` (`Documentation validation`) builds it and binds it to its commit
(`loom-docs provenance`), and on a bot push to `main` `b10x-docs-site.yml` (`Documentation site`)
deploys it to <https://beyond10x.github.io/loom/> through Website's `project-site.yml`.

Every command on a page is run in the tree before it is written, with its output pasted from that
run. Never make a live model call for a page; cite a record under `docs/qualification/`.

## Never

- Never make a model or network call in a test (tests use recorded responses and local fixtures),
  and never for a documentation page.
- Never read a credential file in Loom code or tests.
- Never edit a generated file by hand, and never write an `UNMAPPED:` marker into `ess/`.
- Never implement a story while either ESS gate is red.
- Never add a dependency from `b10x-loom-commission` to the executor, Canon or a model-provider
  crate.
- Never let the slice or the command line merge, push or deploy.
- Never commit or push as anything but the bot, and never use a `gh` write.
- Never write a `/home/<name>/` path literal anywhere; write `~/`.
