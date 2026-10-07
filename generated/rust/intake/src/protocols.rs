// generated from intake v1
// model digest b3281773ee88313834099ce04aab0b9af436baedbe62ff1a432c305c3ed756d4
// contract digest eba8d734a7767f27148ac6b7d3a96ad427989af6d293d3c3496fba405caae95a
// do not edit: regenerate with `ess synthesize --layout crate`

//! protocols — `intake.protocols`.
//!
//! A trusted host composes engineering, Loom and explicitly installed custom Canon definitions into one immutable catalog per run. Registration names are name@major and their major matches the definition revision. Canon owns parsing, validation and evaluation. Routing, artifact initialization and governor admission use the same exact definitions. Duplicate identities refuse, including attempts to shadow a bundled definition. Definition availability does not grant authority or supply executable tools; host bindings and revision initializers decide execution support. Custom documents never execute code or prescribe host commands. Installation snapshots one local file or regular Git blob at a full pinned commit. Git uses HTTPS, SSH or file transports, never a checkout, filters, submodules or package hooks. Installation validates before atomically publishing in the user XDG data directory. Replacement is explicit; bundled entries cannot be replaced or removed. Source bytes and digest are retained together. Every run verifies installed content and loads offline; no automatic network fetch or workspace discovery occurs. Missing, corrupt or incompatible definitions fail before model calls. Source credentials are never stored in locators. Protocol documents are bounded to one MiB and model requests retain the bounded-context ceiling without silently omitting entries. Installed definitions are one atomic manifest bounded to 64 MiB. An existing installation directory without its manifest is corrupt, not an empty catalog. Writers serialize updates; readers observe a whole manifest. Symlinked installation roots or ancestors refuse.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ProtocolDefinition — `intake.protocols.ProtocolDefinition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDefinition {
    /// `name` — `String`.
    pub name: String,
    /// `yaml` — `String`.
    pub yaml: String,
    /// `sha256` — `String`.
    pub sha256: String,
    /// `source` — `intake.protocols.ProtocolSource`.
    pub source: ProtocolSource,
}

/// ProtocolSource — `intake.protocols.ProtocolSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSource {
    /// `kind` — `intake.protocols.SourceKind`.
    pub kind: SourceKind,
    /// `location` — `String`.
    pub location: String,
    /// `revision` — `String`.
    pub revision: String,
    /// `path` — `String`.
    pub path: String,
}

/// SourceKind — `intake.protocols.SourceKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// `Engineering`.
    Engineering,
    /// `Loom`.
    Loom,
    /// `Local`.
    Local,
    /// `Git`.
    Git,
    /// `Memory`.
    Memory,
}
