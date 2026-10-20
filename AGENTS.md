# AGENTS.md — loom

This file is for an agent changing the repository. What Loom is, how to install it and how to embed
it are in [README.md](README.md) and on the site, <https://beyond10x.github.io/loom/>; neither is
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

Loom owns the model/tool loop (ADR 0071): prompt and context assembly, model turns, the projected
action catalogue, action selection, argument generation, tool round trips, streaming, compaction,
sessions and transcripts, budgets, interruption and recovery, and model-facing approval suspension.
Since ADR 0090 it also holds Commission's contracts and runtime, the governor and intake (sections
below).

It does not own protocol semantics (Canon), engineering protocol definitions
(`b10x-canon-engineering`, beyond10x/engineering-protocols), case truth, organizational authority,
connector credentials, final completion, provider wires or credentials (llm), or the engineering
record of a case (AEP).

`beyond10x/harness` is the predecessor. Port its implementation instead of rewriting from zero, and
do not break its consumers while they still depend on it. Code ported from Harness into Loom is
Apache-2.0 (operator, 2026-10-04); Harness keeps its own `LicenseRef-B10x-Proprietary` licence.

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
| The catalogue follows the frontier | `crates/loom-executor/tests/frontier_projection.rs` (`projection_follows_frontier`), `crates/loom-executor/tests/harness_loop_port.rs` (`ported_loop_round_trip`) |
| A selector cannot leave the catalogue | `crates/loom-executor/tests/action_selector.rs` (`selector_cannot_leave_catalogue`) |
| The reasoning-model selector offers the model only its candidates, through any `ModelPort`, and an answer outside them is never a selection | `crates/loom-executor/tests/reasoning_model_selector.rs` (`an_action_outside_the_candidate_set_is_a_selection_error_and_never_a_selection`) |
| Low confidence falls back to the stronger selector, at a host-supplied threshold compared as a number | `crates/loom-executor/tests/confidence_fallback.rs` (`the_fast_choice_is_returned_only_at_or_above_the_supplied_threshold`) |
| A confidence fallback is recorded as two selections, each with its own selector's strategy: the fast one `Overruled`, naming its replacement, which alone reaches argument generation and revalidation; a replacement that is unknown, the selection itself, no longer `Selected` or from another catalogue is refused and nothing is recorded; `Loom::select` returns the pick under its own selector's strategy | `crates/loom-executor/tests/fallback_selection_recording.rs` (`an_overruled_fast_selection_is_recorded_overruled_naming_its_replacement`, `overrule_selection_naming_an_unknown_replacement_is_refused_with_selection_not_found`); `crates/loom-executor/tests/adversary_w4_fallback_overrule.rs`; `crates/loom-executor/tests/adversary_w4_fallback_pipeline.rs` (`adversary_w4_loom_select_carries_the_strategy_of_the_selector_that_made_the_pick`) |
| Every selection gets one `SelectionRecord` equal to it, with `fell_back_to` on the overruled fast selection naming the replacement's strategy; a selection refused at the execution boundary raises its session's `boundary_refusals` by one and adds no record; neither is evidence | `crates/loom-executor/tests/selection_telemetry.rs` (`selection_telemetry_is_recorded`) |
| Laya selects behind the confidence fallback; arguments are generated once, only for the finally selected action, and an action outside the frontier never reaches argument generation or revalidation | `crates/loom-selector-laya/tests/laya_arguments_slice.rs` (`a_choice_outside_the_frontier_is_rejected_and_never_reaches_arguments_or_revalidation`) |
| A blocked or merge-seeking pick is never proposed | `crates/loom-executor/tests/adversary_executor_admission.rs` |
| A workflow asks a model only in a judgement step, offering it only that step's actions; a step whose action the frontier does not list ends `NeedsHumanJudgment` naming it and nothing is proposed; a step advances only once its request was `Performed`; a definition naming an action outside its protocol is refused at load, naming the step | `crates/loom-executor/tests/workflow_executor.rs` (`deterministic_steps_call_no_model`, `a_judgement_step_offers_the_model_only_its_actions`, `a_step_the_frontier_does_not_list_ends_needing_human_judgment`, `a_step_advances_only_after_its_effect_was_performed`, `a_definition_naming_an_action_outside_its_protocol_is_refused`) |
| A later step's arguments carry an earlier step's typed output with no model call, and a missing or mistyped output, or one its step did not produce, proposes nothing and ends `NeedsHumanJudgment` naming the step and the output; a reference to a later, unknown or undeclared output is refused at load | `crates/loom-executor/tests/workflow_step_outputs.rs` (`a_later_step_reads_an_earlier_output_without_a_model`, `a_mistyped_output_ends_needing_human_judgment`, `an_absent_output_ends_needing_human_judgment`, `a_reference_to_a_later_or_unknown_step_is_refused_at_load`, `outputs_are_identical_on_a_rerun_with_the_same_reports`) |
| A judgement step sees exactly the earlier outputs it lists, typed, in one data block of its instructions that no value can close, never as instructions; a listed output the run did not produce asks the model nothing and ends `NeedsHumanJudgment` naming the step and the output; an input naming no earlier declared output, or listed twice, is refused at load | `crates/loom-executor/tests/workflow_step_outputs.rs` (`a_judgement_step_reads_a_listed_output_as_data`, `a_judgement_step_listing_an_absent_output_asks_no_model`, `a_judgement_step_input_naming_no_earlier_output_is_refused_at_load`) |
| A workflow run's record, written after every performed step to the governor's `FileWorkflowRunStore` keyed by the Run id, lets `resume(run_id)` in another process continue the run after an approval or a crash between steps, every output bound, no performed step invoked again, and the usage report covering every segment; a record of another Run is refused, never read | `crates/loom-commission-testkit/tests/workflow_run_store.rs` (`a_step_result_survives_a_restart_across_an_approval`, `the_usage_report_covers_every_segment_of_a_resumed_run`, `a_step_result_of_another_run_is_refused`, `a_workflow_resumes_after_a_crash_between_steps`) |
| A governed run stopped by a budget of its model loop is `Suspended` with `LoopBudget` carrying the loop's stop, through `Loom::run_loop`, `LoopExecutor` and a workflow's judgement step, never `NoUsefulAction` | `crates/loom-executor/tests/budget_stop_cause.rs` (`a_governed_run_stopped_by_its_input_token_budget_reports_the_budget`), `crates/loom-executor/tests/workflow_executor.rs` (`a_judgement_step_stopped_by_its_input_token_budget_suspends_the_run_for_it`) |
| A stale or unlisted request is refused at revalidation | `crates/loom-commission-testkit/tests/action_request.rs`, `crates/loom-executor/tests/adversary_run_revalidation.rs`, `crates/loom-executor/tests/selection_revalidation.rs` |
| Unknown capabilities and panicking authority providers yield no grant | `crates/loom-commission-testkit/tests/adversary_authority_fail_closed.rs` |
| An authority provider is asked about the action request the runtime revalidated, at the run loop and on a resume, bound to the frontier's revision and never to one the arguments name; a grant bound to one revision allows no other; the runtime reads the case's revision again after the allow and once more right before a resumed request is invoked, and performs nothing on a move it sees: a resume that sees it before its take takes nothing, one that sees it after releases the Suspension; a move while the effect itself runs is the effect port's to refuse, by the request's `expected_case_revision`; a failing provider grants nothing at either check | `crates/loom-commission-testkit/tests/authority_request.rs` (`the_provider_sees_the_request_it_decides`, `a_grant_bound_to_one_revision_does_not_allow_the_next`, `a_failing_provider_still_grants_nothing`, `a_resume_that_sees_the_case_moved_after_its_take_releases_the_suspension_and_performs_nothing`), `crates/loom-commission-testkit/tests/adversary_w20261020_u2.rs`, `crates/loom-commission/tests/security_w20261020_u2.rs` |
| Commission names no executor, Canon, `model-provider-deny.txt`, Connectors or Mandate crate | `task commission:deps-guard` (`crates/loom-commission-testkit/tests/executor_port.rs`) |
| A Mandate authority provider grants only on Mandate's allow: a Mandate outage, an authority context that is no Mandate context and the empty case id grant nothing, every outcome answers as `loom.authority` declares, and each case id is its own Mandate resource, `case-<n>` included, so a grant on one case answers for no other | `crates/loom-mandate/tests/mandate_authority.rs` (`an_unavailable_mandate_grants_nothing`, `an_authority_context_that_is_no_mandate_context_grants_nothing`, `every_outcome_answers_as_the_specification_declares`, `a_governor_case_id_is_a_mandate_resource`, `distinct_case_ids_are_distinct_resources`), `crates/loom-mandate/tests/adversary_w10_u2.rs`, `crates/loom-mandate/tests/security_w10_u2.rs` |
| With `--mandate`, a resume performs the held request only when Mandate's `check_with_approval` clears and consumes the approval the grant names, kept in the state directory's log before anything is performed: a second resume with it, an expired approval, one naming another subject, action or resource, one recorded for another commission however its id is written, and a grant naming none are refused and perform nothing; without `--mandate`, a grant naming an approval is refused and performs nothing; the approver is neither the verified subject nor its actor; only `b10x-loom-cli` links `b10x-loom-mandate`, and only it and `loom-docs` reach Mandate over `Cargo.lock` | `crates/loom-cli/tests/resume_after_grant.rs` (`a_recorded_approval_clears_the_held_request_once`, `a_second_resume_with_the_same_approval_is_refused_and_performs_nothing`, `an_expired_approval_is_refused_and_performs_nothing`, `the_grant_names_its_mandate_approval`), `crates/loom-cli/tests/adversary_w10_u3.rs` (`adversary_a_resume_without_mandate_does_not_perform_a_grant_naming_a_mandate_approval`), `crates/loom-cli/tests/security_w10_u3.rs` (`security_an_approval_is_bound_to_its_commission_however_its_id_is_written`), `crates/loom-mandate/tests/mandate_approval.rs` (`an_approval_naming_another_subject_action_or_resource_is_refused`), `crates/loom-mandate/tests/adversary_w10_u3.rs` (`adversary_a_host_file_whose_approver_is_the_acting_actor_is_refused`), `crates/loom-mandate/tests/security_w10_u2.rs` |
| An executor's report that the case moved is decided by the governor and judged on the case's current frontier, reloaded once | `crates/loom-commission-testkit/tests/moved_case_outcome.rs`, `crates/loom-executor/tests/adversary_w1_runtime_stale.rs`, `crates/loom-executor/tests/adversary_w1p2_runtime_windows.rs` |
| An executor is never handed an unperformed action that needs no authority; an effect is invoked once, through its binding, and again only after the port answered `Pending` | `crates/loom-commission-testkit/tests/effect_invocation.rs` (`effect_invoked_only_through_its_binding`) |
| An effect port's `Pending` answer gets its request invoked again at the next iteration, under its own id and at its own revision, with no executor call between, revalidated before each invocation as before the first (authority asked again, the revision read again after an allow), until the port answers otherwise: a request whose case moved, that the frontier no longer admits or the provider denies is not invoked again, and one the provider answers `ApprovalRequired` for ends the run `NeedsAuthority`, holding a Suspension under `run_with_suspension`; a pending read of a fan-out is invoked again alone, and only while the fan-out rule still admits it; each invocation is a step and one trial, each answer one observation and never evidence; a pending answer neither joins nor restarts the idle count; a budget that ends the run while a request is pending ends it `Waiting`, naming the requests and the budget's reason, the Run suspended for it; the slice stops it `StepBudget` | `crates/loom-commission-testkit/tests/pending_effect.rs` (`a_pending_answer_is_invoked_again_and_not_counted_idle`, `a_pending_answer_leaves_the_idle_count_where_it_was`, `each_reinvocation_is_revalidated`, `a_budget_reached_while_pending_ends_the_run_waiting`, `each_answer_is_charged_and_observed`, `approval_required_when_a_pending_request_is_asked_again_ends_the_run_needing_authority`), `crates/loom-commission-testkit/tests/adversary_w20261020w3_u2.rs`, `crates/loom-intake-slice/src/run.rs` (`a_run_waiting_on_a_pending_request_stops_step_budget`), `task commission:conform` (`JudgeBudgetStop`) |
| An effect refusal marked final (`ends_run`) ends the run at once with `EffectRefused`, naming the request and the reason, once it was recorded and observed: after a proposal, after a fan-out once every read was recorded (beside a stale read it is kept until the move is judged, and ends the run when the next load finds no move), and after a resumed request, whose Suspension is released; an ordinary refusal still counts idle | `crates/loom-commission-testkit/tests/refused_effect_outcome.rs` (`a_final_refusal_ends_the_run_naming_request_and_reason`, `an_ordinary_refusal_still_counts_idle`, `the_refusal_is_observed_once_and_recorded`), `crates/loom-commission-testkit/tests/adversary_w20261020w2_u1.rs`, `task commission:conform` (`JudgeEffectRefusal`) |
| Without a step budget, two idle iterations in a row end the run `IdleBound`, naming them, whatever kind of idle iteration they are, a repeated admission included, and a new admission starts the count again; a case the last one's performed effect completed ends the run completed instead, the case loaded first as at the step budget; a frontier that admits nothing still ends `NoAdmissibleAction`; the slice stops `IdleBound` `NothingAdmissible`, as before | `crates/loom-commission-testkit/tests/idle_bound_outcome.rs` (`two_idle_iterations_end_the_run_as_the_idle_bound`, `a_frontier_that_admits_nothing_ends_as_no_admissible_action`, `a_repeated_admission_counts_toward_the_idle_bound`, `a_case_its_repeated_reads_completed_ends_the_run_completed`), `crates/loom-commission-testkit/tests/adversary_w20261020w2_u2.rs`, `crates/loom-intake-slice/src/run.rs` (`an_idle_bound_run_stops_nothing_admissible_as_before`), `task commission:conform` (`JudgeIdleIteration`) |
| A run the effect port performs nothing for ends `NoPerformableAction` naming the actions it does not perform, each once, in frontier order, and the frontier's open claims (its claims not `True`, which the frontier relates to no action): before the executor runs, after an admitted proposal (that action alone) and after a move, one the governor bears out or one the runtime finds after a stale proposal; the slice's `NoLocalExecutor` stop carries the actions, and its `run --output jsonl` terminal record names them as `unperformed_actions` | `crates/loom-commission-testkit/tests/unperformable_names.rs` (`the_outcome_names_each_unperformed_action_and_its_claim`, `a_proposed_unperformed_action_is_named`, `an_action_listed_twice_is_named_once`, `a_moved_case_whose_frontier_performs_nothing_names_its_actions`), `crates/loom-commission-testkit/tests/adversary_w20261020w3_u1.rs` (`adversary_a_found_move_onto_nothing_performable_names_the_actions`), `crates/loom-cli/tests/run_events.rs` (`a_no_local_executor_stop_names_its_actions`), `task commission:conform` (`JudgePerformableActions`) |
| Reads proposed together (`ProposedReads`) are admitted only when the effect port reads every one and the handed frontier lists each `Admissible` with no authority, else nothing is invoked; admitted reads are revalidated each, invoked concurrently, each once through its binding, joined, and recorded in proposal order; a stale read alone is not invoked; an effect error fails the loop after every read joined; a fan-out is one step | `crates/loom-commission-testkit/tests/read_fan_out.rs` (`admitted_reads_run_concurrently_and_join`, `effects_are_recorded_in_proposal_order`, `a_fan_out_with_a_write_or_an_approval_required_read_invokes_nothing`, `a_stale_read_is_refused_and_the_others_are_performed`, `an_effect_error_fails_the_loop_after_every_read_joined`), `crates/loom-connectors/tests/read_fan_out.rs` (`each_read_is_invoked_once_through_its_binding`) |
| A consequential action leaves Loom only as a proposal, and the executor links no Connectors crate | `crates/loom-executor/tests/connector_boundary.rs` (`consequential_actions_leave_loom_only_as_a_proposal`) |
| A Connector invocation is made once, and only Connectors' own "nothing changed" is a refusal | `crates/loom-connectors/tests/connectors_invoker.rs` |
| A write through the Connectors command line runs only for an admitted request and is bound to its admitted action and arguments, with an absolute approval proof path passed and never read, and an idempotency key derived from the request id; another action, input other than the admitted arguments, an operation not described as a write, or a missing, relative or control-character proof path, invokes nothing; an `unknown` or lost write is returned unknown and never invoked again; reads still refuse a write; only an applied write of the admitted request becomes the protocol's evidence, naming its step and, for a route, its target (the `queue` or `owner` the admitted arguments name, which the governor removes before Canon reads the record); a route whose target cannot be read, or whose answer names another one, records nothing and says why | `crates/loom-connectors/tests/connectors_cli_write.rs` (`an_admitted_write_passes_the_proof_path_and_the_derived_key`, `a_write_without_an_absolute_proof_path_starts_nothing`, `an_operation_not_described_as_a_write_is_refused_before_the_invoke`, `the_same_admitted_request_yields_the_same_idempotency_key`, `an_unknown_write_is_returned_unknown_and_never_invoked_again`, `a_write_whose_answer_is_lost_is_unknown`, `invoke_read_still_refuses_a_mutation_operation`), `crates/loom-connectors/tests/support_triage_route.rs` (`an_applied_route_records_a_routing_record_naming_the_step`, `the_route_is_not_admitted_before_the_review_approved`, `a_route_not_applied_records_no_evidence`), `crates/loom-connectors/tests/adversary_w172_governed_write.rs` (`input_the_runtime_did_not_admit_is_never_written`, `a_request_admitted_for_another_action_writes_nothing`, `a_control_character_proof_path_starts_no_process`, `the_adapter_refuses_the_write_of_another_admitted_request`, `the_record_names_the_target_the_admitted_arguments_name`, `a_route_whose_target_cannot_be_read_records_nothing_and_says_why`, `an_answer_naming_another_target_records_nothing`), `crates/loom-governor/tests/adversary2_governor.rs` (`a_target_in_the_facts_leaves_the_record_as_canon_evaluates_it`) |
| A plugin turn reads only the sources and kinds of its projection, and a proposal is recorded, never sent | `crates/loom-plugin/tests/effects.rs` (`effects_refuse_an_undeclared_read`, `propose_writes_the_record_only`) |
| A slack-handler run describes and invokes only the three configured Slack reads and the turns' data-source reads, and records one line per item it proposes for; nothing is sent | `crates/loom-plugin-slack/tests/run.rs` (`fixture_run_invokes_no_write`, `fixture_run_records_three_proposals`) |
| A read through Connectors is performed naming its audit record; a write without a recorded attempt is an error | `crates/loom-connectors/tests/connectors_invoker.rs` (`an_admitted_read_is_performed_with_its_result_and_names_its_audit_record`, `a_write_without_a_recorded_attempt_is_still_an_error`), `crates/loom-commission-testkit/tests/effect_invocation.rs` (`a_performed_that_names_another_record_than_its_binding_requires_is_not_answered`) |
| The executor answers the commands of `ess/` as their synthesized scenarios specify, except `not-in-frontier`, which the conformance target answers | `task conform` (`crates/loom-conformance/tests/conform.rs`, `ess_conformance_report`); the membership rule: `crates/loom-executor/tests/adversary_run_revalidation.rs` (`not_in_frontier_follows_the_frontier_actions`) |
| The intake slice answers the commands of `ess/intake/` as their synthesized scenarios specify: a resume context is kept once (`KeepResumeContext`) and dropped (`DropResumeContext`), a merge hand-off is recorded once per request and per `HEAD` (`RecordMergeHandoff`), and the stores read back what the views `ResumeContexts` and `MergeHandoffs` publish | `task intake-conform` (`crates/loom-intake-conformance/tests/conform.rs`, `ess_conformance_report`) |
| The router refuses a pick outside the registry or below the threshold | `crates/loom-intake-router/tests/adversary_classify.rs` |
| A metered run reports one model call per `ModelPort::turn`, failed turns included, one step per executor call, and tokens and cost per step that sum to the run's; a cost no rate card priced is absent, never zero; the usage an executor that is not a model loop reports (`AgentExecutor::run_with_usage`) is its step's usage and no model call, never in the report's effects, and is in the meter only when the call came through `run_with_usage`, so the meter and the run's charges agree; an executor metered at its model port reports none, so nothing is counted twice | `crates/loom-executor/tests/run_metering.rs` (`the_reported_calls_equal_the_turns_the_model_port_saw`, `a_step_counts_the_selector_and_argument_calls_made_in_it`, `cost_is_absent_without_a_rate_card_and_for_an_unlisted_model`, `a_non_model_executor_usage_is_its_step_usage`, `a_step_sums_its_metered_turns_and_the_usage_its_executor_reports`, `an_executor_metered_at_its_model_port_reports_no_usage_and_is_counted_once`), `crates/loom-executor/tests/adversary_w20261020w3_u3.rs` (`a_wrapper_that_does_not_forward_leaves_the_meter_and_the_budget_agreeing`) |
| An executor call's reported usage (`ExecutorStep.usage`) is charged to the run, never to an action or as a trial, and delivered in the step's observation; a usage with no cost leaves the run's cost absent; a charge past a spending limit suspends the Run with `RunBudget` once the step was observed, before its outcome is acted on | `crates/loom-commission-testkit/tests/executor_usage.rs` (`an_executor_usage_is_charged_to_the_run`, `a_run_budget_breach_by_executor_usage_suspends_the_run`, `an_unknown_cost_stays_unknown`) |
| Model or provider JSON nested past 128 levels is refused | `crates/loom-executor/tests/json_depth.rs` |
| No hand-written type shadows an ESS-declared one | `task no-hand-model`, `task commission:no-hand-model` |
| Ported Harness modules keep the import limits their crates had; `turn_loop` alone may also name the generated model (`crate::model`), for `LoopStop` | `crates/loom-executor/tests/adversary_harness_port_boundaries.rs` (`the_boundaries_harness_enforced_by_crate_still_hold_between_modules`), `crates/loom-executor/tests/adversary2_harness_port.rs` (`each_ported_module_names_only_what_its_harness_manifest_allowed`) |
| Package and library names are the `loom-` names | `crates/loom-executor/tests/crate_names.rs` |
| No manifest names an archived Commission, governor or intake repository | `crates/loom-executor/tests/governor_import.rs`, `intake_import.rs`, `commission_import.rs` |
| The SDK stability page (`website/docs/reference/sdk-stability.md`) lists exactly the ports a host implements and the `loom_sdk` paths control-plane imports, each resolving through `loom_sdk`; a change to that surface updates the page and names any break under `Breaking:` in `CHANGELOG.md` | `crates/loom-sdk/tests/host_ports.rs` (`a_host_implements_every_port_through_loom_sdk_alone`, `the_stability_page_lists_exactly_the_ports_this_test_implements`, `every_control_plane_path_on_the_page_resolves`) |
| The `run --output jsonl` records and exit statuses change within `schema_version` 1 only by a new optional field; any other change fails the gate until the committed contract moves | `task docs-check` (`crates/loom-docs/src/run_events.rs`, `the_v1_contract_matches_the_declared_events`, `a_removed_renamed_or_retyped_field_is_a_break`, `a_new_record_kind_is_a_break`, `a_new_optional_field_is_not_a_break`) |
| A release tag equals the workspace version and has a CHANGELOG entry | `loom-xtask release-check`, run by `release.yml` (`crates/loom-xtask/tests/release_check.rs`) |
| A release's CHANGELOG entry says what breaks for a consumer: it has a `Breaking:` sentence, and `Breaking: none.` is refused when `consumers/pinned/`, `crates/loom-sdk/tests/host_ports.rs` or `docs/contracts/run-events-v1.json` changed since the previous tag; `release.yml` fetches the history and tags it diffs | `loom-xtask release-check` (`crates/loom-xtask/tests/release_check.rs`: `release_check_names_an_entry_without_a_breaking_sentence`, `release_check_refuses_breaking_none_when_a_stable_surface_changed_since_the_previous_tag`, `release_check_accepts_breaking_none_when_no_stable_surface_changed`, `release_check_passes_on_the_first_tag`, `release_workflow_verifies_the_tag_and_reuses_the_check`) |
| The documented embedding builds and runs to `ApprovalRequired (repository.merge)` as an external crate: `consumers/pinned/` is its own workspace, depends on `b10x-loom-sdk` alone by path, includes `crates/loom-sdk/examples/software_change.rs` by `#[path]` and resolves one copy of each Loom and Canon crate | `task consumer-check` (`consumers/pinned/tests/embedding.rs`, `the_pinned_consumer_stops_at_approval_required`), `crates/loom-xtask/tests/pinned_consumer.rs` |

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
with the tests open. CI installs `ess` 0.60.0 (`.github/workflows/check.yml`); move that pin when a
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
   `cargo test --workspace` does not reach it.
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
capability updates it, the page that describes it and `CHANGELOG.md` (**Unreleased**) in the same
commit. A change to a command, a crate or a rule updates README.md and this file in that commit too.

## Commission

Commission's history was merged into Loom, and its files keep their Commission paths so
`git log --follow` reaches it. Packages: `b10x-loom-commission` (the contracts and runtime, with
`run_until_blocked`), `b10x-loom-commission-testkit` (port fakes and adapter kits),
`b10x-loom-commission-conformance` (holds the contracts crate to its synthesized suite),
`loom-commission-docs` and `loom-commission-xtask`. A run suspended on a withheld approval
outlives its process only through the suspension port the host implements
(`ports::suspension`, `run_with_suspension`, `resume_after_grant`); `run_until_blocked` takes
none. The port answers the Suspension's commands (`HoldSuspension`, `TakeSuspension`,
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
(`crates/loom-governor/tests/explicit_decisions.rs`).

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
outlives the read. A Connectors refusal is a
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

## Plugin host

`b10x-loom-plugin` (`crates/loom-plugin`) hosts a plugin (ADR `plugin-hooks`): a crate linked at
build time implementing `Plugin` (`poll`, `classify`, `project`, `turn`, `result`, `objectives`),
run by `run_plugin(plugin, config, state, stop)`. Its nouns are the `loom.plugin` domain
(`ess/domains/plugin.yaml`); a plugin's own configuration extends `PluginConfig`. The plugin hooks
sit outside a run and grant nothing. A turn is a thin caller of `run_until_blocked` through
`loom_sdk`, on `inbound-answer@1` from `ProtocolCatalog::plugins()`, with Loom's governed model loop
(`LoopExecutor`), `PluginAuthority` (grants `datasource.read` and `reply.propose`, denies every
other capability) and `DataSourceEffects`, which reads through `loom_connectors::cli`, refuses a
`{source, kind}` outside the projection before any command, and submits one `source_read` evidence
per performed read. A turn is one Run, bounded by `TURN_STEP_BUDGET`: a step Commission performed
under a grant passes its approval gate (`docs/commission/design/approval-gate.md`), so the Run goes
on after each granted read, and an `AwaitingApproval` ends the turn `stopped`. Model requests are
bounded per turn: `TURN_MODEL_BUDGET` counts every request sent, wire retries included; the port
the turn lends refuses any past it. A classifier hint that
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
`crates/loom-plugin/tests/` use a recorded classifier, a scripted model port and the fake
`connectors` of `loom-connectors`. The command line hosts registered plugins through
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
cursor never passes a message too young to judge. The tests in `crates/loom-plugin-slack/tests/`
reuse `crates/loom-plugin/tests/support/mod.rs` by path and answer Slack-shaped JSON from
`tests/fixtures/` through the fake `connectors`; ids there are synthetic.

`b10x-loom plugin run` settles both models before any read: with `--catalog` from route aliases
(`model_catalog`), otherwise Codex models, as `run` does. A turn runs on
`loom_sdk::llm::LlmModelPort` (`crates/loom-sdk/src/llm.rs`), the bridge from an llm `Model` to
the harness `ModelPort`: one `Model::turn` per port turn, no retry and no other target, an opaque
item carried back under the wire id `llm-model` only. The CLI defines no model port of its own
(`crates/loom-sdk/tests/llm_model_port.rs`). Without `--once`, the first SIGTERM or SIGINT sets the host's stop, so
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

## Workflow definitions

`b10x_loom_executor::workflow` (`story:dw-workflow-definition-executor`) runs a Loom workflow
definition, the `loom.workflow` domain (`ess/domains/workflow.yaml`), as Commission's
`AgentExecutor`. A definition names its protocol by catalog reference and orders its steps: a
deterministic step (one `action`, literal `arguments`) is proposed without a model; a judgement
step runs the governed loop (`LoopExecutor`) narrowed to its `actions` that the host's own
narrowing (`LoopConfig::admits`) admits, never widening it, with its `instructions` as the run's
instructions; `when` makes a step wait on a frontier claim, never on a model.
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
declares, introduced as data and never as instructions; `<`, `>` and `&` inside it are written as
`\u` escapes, so no value can close the block. The model never reads them from the transcript, and
no model supplies an output. A listed output the run did not produce (its step was skipped) or one
not of its declared type ends the call `NeedsHumanJudgment` naming the judgement step and the
output, and asks the model nothing. `load` refuses, naming the judgement step, an input naming a
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
  `website/docs/reference/run-events.md` against the declaration.
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
  which it is killed and reaped: exit 0 is `Performed` with its stdout (one JSON object) as the
  report, anything else `Refused`. Evidence comes only from the report's `evidence` list
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

The injected `TestRunner` defaults to Substrate 0.7.17, with host and wire pinned together.
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
`SandboxProfile` and `NetworkMode`, `NetworkMode::None` by default, or with `with_egress` the one
Substrate `EgressAperture` it may reach, and with `with_ca_bundle` the certificate bundle it
trusts there) on the same launch path:
writes only under `target/` and the directories named (Substrate's `WorkspaceAccess::Scoped`;
`.git` only when named; a path that is absolute, climbs out with `..`, passes a symlink, is absent
or no directory, holds a special file or a hard link from outside, or overlaps another is refused
`ScopeInvalid` before launch; `TestRunner::run` keeps `target/` alone, and the applied
confinement's `writable_scopes` lists them, a record naming another set refused), no toolchain or
cache mounted, exactly the named environment (with a bundle, plus `SSL_CERT_FILE`), a typed
refusal for a missing guarantee, only the requested network accepted as applied, no network or
exactly the named aperture (name, pinned destination, mechanism, ceiling), checked after the child
ran and refusing its result as a test's is refused, a result with no exit status refused unless
its timeout or its aperture's byte ceiling (`exec.aperture-byte-limit`, named in its output, its
driver record counting the bytes) ended it, and the applied confinement in its `TestExecution`,
whose `network` names an aperture `aperture:<name>@<destination>`. The child reaches the aperture
by the host's name, which Substrate maps to its forwarder on `127.0.0.1` inside the sandbox. An
aperture declaration Substrate's daemon never makes (no host or port, a name it refuses, a zero
byte ceiling, a host that is no host name or IPv4 literal, an IPv4 literal other than the pinned
address, a pinned address that is no IPv4 one or not at the named port), and an IPv4 literal host
other than `127.0.0.1`, which the child could not reach the host by, are refused `ScopeInvalid`
before anything starts, the refusal naming the host; one Substrate did not publish when the driver opened (its mechanism unproven) is refused
`CapabilityUnserved`. Substrate gives a run at most one aperture. A certificate bundle
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
bounded by the aperture. A bundle for a child with no aperture, a path that is not UTF-8 or not
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
assignment or an option. A runner and its clones share one driver per workspace directory, egress
aperture and certificate bundle, the bundle compared as its path and the bytes read
(`DriverEgress`: Substrate fixes a driver's apertures and bundle when it opens), so children naming
different bundles, or a bundle whose bytes changed, never share one,
because opening a driver kills every `substrate-ex_*` cgroup under its root; each launch discards
its finished execution from the driver (`discard_superseded_exec`), which tracks at most 128; no
driver opens on a root while a launch of this process runs there (the process-wide count in
`confinement.rs`, keyed by the canonical root), so the launch needing it is refused instead.
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
cancellation-aware backoff and per-attempt context metrics. Model ports stay single-attempt;
effects remain outside this loop. `system_query.rs` exercises recovery in all three request phases. The Codex preset and the forced tool call are
`codex_model` (the CLI builds its model with it) and `call_tool` (the router calls it). With
`run --catalog`, the CLI builds both models instead with `b10x-llm-models` `port` from the
route aliases `--model` and `--classifier-model` name (`crates/loom-cli/src/model_catalog.rs`,
held by `crates/loom-cli/tests/catalog_route.rs`): one target per route, no fallback, and no
credential resolver, so a credentialed account is refused before any model call. The
credential is the operator's Codex login (`~/.codex/auth.json`), read and renewed by llm, never by
Loom code. The ported wires take theirs from a `loom.run.WireCredential` reference through
`credentials::ResolvedBearer`, which asks a `SecretResolver` the embedder injects at each call.

The SDK ships two model ports. `loom_sdk::llm::LlmModelPort` is the llm-backed one above.
`loom_sdk::loom::wire::ClaudeCodePort` (`crates/loom-executor/src/wire/claude_code.rs`) runs on an
operator's Claude Code login only through the `claude` command, which reads that login itself:
one turn is one `claude -p --output-format json` invocation, the command's own tools off, the
turn's tools and conversation on its standard input as escaped JSON blocks, and a call answered
as a JSON object the loop executes. It is single-attempt, bounded in output and time; a missing
or failing command is a `WireError` naming it. A port that reads a Claude Code login directly is
llm's to build, never Loom's. `crates/loom-sdk/tests/sdk_model_ports.rs` runs a governed loop turn
on it over a stand-in `claude` on `PATH`. The ported `harness::wire` stays free of I/O; Loom's own
ports that run something are under `wire`.

## Planning and waves

- The plan is the AEP store under `.engineering/`, written only through `aep plan artifact`. Body
  drafts go in `.engineering/drafts/` (git-ignored). Draft stories are planned, never shipped.
- A wave runs on `wave/<date>-w<n>`: `plan: open wave … (story:…)`, one `impl/<story>` branch per
  story merged into the wave branch, then `plan: close wave …`, and `main` fast-forwards to the
  wave head. Work outside a wave lands as a bot pull request.
- Use a managed worktree (`worktree create --repo ~/beyond10x/loom --purpose …`); keep the primary
  checkout clean.

## Commits, pushes and GitHub writes

Every commit and push is `b10x-bot[bot]`'s through `b10x-gates bot`. Every other GitHub write
(pull request, comment, issue, release, workflow dispatch, re-run) goes through `b10x-gates api`.
`gh` is read-only here. Loom is enrolled in common Gates (`shared-gates.yml`); its
`common / Security and privacy` check scans every commit since the policy baseline, so a
`/home/<name>/` path literal in any commit fails it and admits no exception.

## Releases

Loom releases from source at bare-version tags (`0.1.0`, no `v`); every crate inherits the
workspace version and nothing is on a registry. Consumers pin `tag = "<version>"`, and
`CHANGELOG.md` collects under **Unreleased** between releases. A release is:

1. A release commit on `main` (through a wave or a bot pull request): `[workspace.package] version`
   in `Cargo.toml`, `Cargo.lock`, and **Unreleased** turned into `## [<version>] - <date>` under an
   empty **Unreleased**, opening with a one-paragraph summary fit for the release notes. README,
   AGENTS.md and the site pages that name the version or a dependency line move with it.
   `cargo run -q --locked -p loom-xtask -- release-check --tag <version>` passes on it.
2. `check` and `Shared source gates` green on that commit.
3. An annotated tag by `b10x-bot[bot]` on that commit: `b10x-gates bot … -- tag -a <version> -m
   "Loom <version>" <commit>`, then `-- push origin <version>`.
4. `release` (`.github/workflows/release.yml`) green on the tag. It runs `loom-xtask release-check`
   (the tag equals the workspace version and `CHANGELOG.md` has its entry, with a `Breaking:`
   sentence that says `none` only when no stable surface changed since the previous tag, so the
   job checks out the full history and its tags) and then `check.yml` on
   the tagged commit. It is read-only, needs no secret and creates no release.
5. The GitHub Release for the tag, created by the bot (`b10x-gates api --method POST --path
   /repos/beyond10x/loom/releases`), named `<version>`, its notes the version's CHANGELOG entry.

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
