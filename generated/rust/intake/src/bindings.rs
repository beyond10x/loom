// generated from intake v1
// model digest cf1fbbfc1206e350609f5f4f3522b47696f03a8049cc32a47b5ed52b150d1d84
// contract digest 7dd547604a8ca8784e13b80a1979b25542e31ca93b347da47fcc3787eeb2950c
// do not edit: regenerate with `ess synthesize --layout crate`

//! Action bindings — `intake.bindings`.
//!
//! The commands an operator binds to the actions of one installed protocol, so that `b10x-loom run` drives a protocol whose actions no built-in binding performs. The operator declares them in a run configuration file (`run --action-bindings <FILE>`): the protocol, by its registration name, and for each action one argv, run without a shell. A run of that protocol performs an admitted action by starting its command with the action's arguments as JSON on standard input; the command's exit status decides the outcome, 0 performed and any other refused, and its standard output, one JSON object, is the action's performed report. The command is the verifier of its own action: its report may state evidence of the kinds the action declares it may produce, and only those are submitted, about the intent. A binding confers no authority: an action that needs a capability still needs its grant, and the governor still decides admission. Commands start with a cleared environment that keeps only PATH, never Loom's own credentials, and are killed past the configuration's timeout. Substrate does not confine them, so a run given a run configuration is refused, before any model call, with the confinement refusal CommandUnconfined unless the operator opts in to running the commands unconfined: with the run's `--unconfined-bindings` flag, or with the file's own `unconfined_bindings: true`; the flag wins, and the run reports which one opted in. A routed protocol that declares an action the file binds no command to ends with no local executor, naming the action. Configuration is trusted host context, never model output, and a protocol document never names a command.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ActionBinding — `intake.bindings.ActionBinding`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBinding {
    /// `action` — `String`.
    pub action: String,
    /// `argv` — `List<String>`.
    pub argv: Vec<String>,
}

/// ActionBindings — `intake.bindings.ActionBindings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBindings {
    /// `protocol` — `String`.
    pub protocol: String,
    /// `timeout_seconds` — `Optional<Integer>`.
    pub timeout_seconds: Option<i64>,
    /// `unconfined_bindings` — `Optional<Boolean>`.
    pub unconfined_bindings: Option<bool>,
    /// `bindings` — `List<intake.bindings.ActionBinding>`.
    pub bindings: Vec<ActionBinding>,
}

/// CommandEvidence — `intake.bindings.CommandEvidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEvidence {
    /// `kind` — `String`.
    pub kind: String,
    /// `result` — `String`.
    pub result: String,
}
