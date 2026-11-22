// generated from loom v1
// contract digest 6b2527fe99745029eca025e9d3d3f5c47942fb32e34b812bfd6d365b205a4e01
// do not edit: regenerate with `ess synthesize --layout crate`

//! Every actor the specification declares, and the commands each may invoke — as data.
//!
//! A grant is checked against a caller identity, which these types do not read from anywhere:
//! whatever authenticates a request builds a [`Caller`], and a served surface checks it against
//! [`may`] before the command runs. The `PLAN.md` beside this workspace says, per actor,
//! whether a generated surface enforces the grant or the caller does.

/// An actor the specification declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Actor {
    /// `loom.decision.Decider`.
    Decider,
    /// `loom.decision.Person`.
    Person,
    /// `loom.decision.Runtime`.
    Runtime,
}

impl Actor {
    /// Every declared actor, ordered by qualified name.
    pub const ALL: &'static [Actor] = &[
        Actor::Decider,
        Actor::Person,
        Actor::Runtime,
    ];

    /// The actor's qualified name, as the specification spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Actor::Decider => "loom.decision.Decider",
            Actor::Person => "loom.decision.Person",
            Actor::Runtime => "loom.decision.Runtime",
        }
    }
}

/// The qualified names of the commands `actor` may invoke, ordered by name; empty for an
/// actor that only observes.
pub fn may(actor: Actor) -> &'static [&'static str] {
    match actor {
        Actor::Decider => &[
            "loom.decision.Decide",
            "loom.decision.Escalate",
            "loom.decision.TakeRecommendation",
        ],
        Actor::Person => &[
            "loom.decision.Decide",
            "loom.decision.Escalate",
            "loom.decision.TakeRecommendation",
        ],
        Actor::Runtime => &[
            "loom.decision.Expire",
            "loom.decision.Recommend",
            "loom.decision.RecordProduced",
            "loom.decision.Request",
            "loom.decision.Supersede",
            "loom.decision.TakeRecommendation",
        ],
    }
}

/// Who a request was authenticated as.
///
/// Built by whatever authenticates the request — a session, a token, a certificate — and
/// handed to the served surface's `dispatch` and `handle`, which check its grant
/// before the command runs. Never derived from the request itself: a client can write
/// anything into a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Caller {
    /// The declared actor.
    pub actor: Actor,
}

impl Caller {
    /// `true` when this caller may invoke `command`, named by its qualified name.
    pub fn may(&self, command: &str) -> bool {
        may(self.actor).contains(&command)
    }
}
