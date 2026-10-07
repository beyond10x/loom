// generated from intake v1
// model digest 8158f2c6f08736511dded8320855abb57ef13c8b3b492ffef5ed2fd4bb05d87d
// contract digest 6edfc386e180a0eea5e77b3cac17ef9f12f629f4d81c0918f6db81cbb71e4848
// do not edit: regenerate with `ess synthesize --layout crate`

//! How the specification's primitives are spelled in this workspace.
//!
//! Four map onto types that already mean exactly the same thing: `String` stays `String`,
//! `Boolean` is `bool`, `Integer` is `i64`, `Bytes` is `Vec<u8>`. The four below have no `std`
//! equivalent, and no dependency is taken for them — this workspace builds from exactly its
//! committed bytes. Each is a transparent wrapper over its wire rendering, distinct from `String`
//! and from each other for the same reason the specification's own newtypes are distinct from
//! their representations: a value's meaning is not its shape.

/// An exact decimal, carried as its wire rendering — a decimal string such as `10.50`.
///
/// Never a float: money does not round the way a float does. Equality and order are over the
/// rendering, so `1.5` and `1.50` are different values here; arithmetic is deliberately absent,
/// because the specification declares no operation on a decimal, so there is none to generate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Decimal(pub String);

/// An instant, carried as its wire rendering — RFC 3339, such as `2026-01-01T00:00:00Z`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub String);

/// A length of time, carried as its wire rendering — an ISO 8601 duration such as `P30D`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration(pub String);

/// A UUID, carried as its canonical textual rendering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Uuid(pub String);
