//! The one integration-test binary of `b10x-loom-intake-slice`: one module per file under `tests/`.

/// The name libtest gives the test `$name` of the calling module, `<module>::<name>`: the name a
/// case that starts this binary again passes to `--exact`, and the name it reads back in the
/// binary's `test <name> ... ok` line.
macro_rules! full_name {
    ($name:expr) => {
        format!(
            "{}::{}",
            module_path!()
                .split_once("::")
                .map_or(module_path!(), |(_, module)| module),
            $name
        )
    };
}

mod adversary2_case;
mod adversary2_w20261020w3_u5;
mod adversary_case;
mod adversary_confinement;
mod adversary_host_git;
mod adversary_pr16_briefing;
mod adversary_pr16_edit_budget;
mod adversary_pr16_lookups;
mod adversary_pr16_reference_cap;
mod adversary_pr16_refusal;
mod adversary_selector_executor;
mod adversary_substrate_0_12_pin;
mod adversary_w10_u1;
mod adversary_w20261020_u1;
mod adversary_w20261020w2_u4;
mod adversary_w20261020w2_u5;
mod adversary_w20261020w2_u6;
mod adversary_w20261020w3_u5;
mod adversary_w20261021w6_u3;
mod adversary_w20261023w2_u1;
mod adversary_w20261023w2_u2;
mod adversary_w20261023w3_u1;
mod adversary_w7_u2_store;
mod adversary_w8_u1_handoff;
mod adversary_w9_u1_inputs;
mod bound_command;
mod bound_command_confined;
mod bounded_context;
mod confined_child;
mod confinement;
mod confinement_spec;
mod engine_confinement;
mod host_clock;
mod host_git_hardening;
mod regression_command;
mod result_reference_workflow;
mod security_w10_u1;
mod security_w11_u5_regression;
mod security_w20261021w6_u2;
mod security_w20261022w1;
mod security_w20261022w2_u1;
mod security_w20261023w2_u1;
mod security_w20261023w3_u1;
mod security_w20261121w1_u1;
mod security_w20261121w1_u1_pass2;
mod security_w20261121w3_u2_approve;
mod security_w20261121w4_u1;
mod security_w20261121w4_u1_pass2;
mod security_w20261121w5_u1;
mod security_w20261121w6_u1;
mod selector_executor;
mod slice_case;
mod software_intent;
mod support;
mod system_query;
