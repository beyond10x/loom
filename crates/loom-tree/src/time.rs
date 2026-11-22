//! Where a tree command reads its instants: the host clock, or the one instant `--at` gives.

use b10x_loom_intake_slice::clock::{SystemClock, instant};
use loom::json::{Value, timestamp_at};
use loom::primitives::Timestamp;
use loom::tree::TreeRefusalReason;

use crate::Refused;

/// The instants of one tree command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeTime {
    /// The host clock, UTC to the second, read at each instant.
    Host,
    /// Every instant is this one, `YYYY-MM-DDTHH:MM:SSZ`.
    At(String),
}

impl TreeTime {
    /// The time of a command given `--at` (`None` for the host clock).
    ///
    /// # Errors
    /// `InvalidInstant` when `at` is not an RFC 3339 instant in UTC to the second,
    /// `YYYY-MM-DDTHH:MM:SSZ`, the form Canon writes.
    pub fn from_flag(at: Option<&str>) -> Result<Self, Refused> {
        let Some(at) = at else {
            return Ok(Self::Host);
        };
        let canonical = at.len() == 20
            && at.ends_with('Z')
            && at.as_bytes()[10] == b'T'
            && timestamp_at(&Value::Text(at.to_owned()), "--at", "an instant").is_ok();
        if canonical {
            Ok(Self::At(at.to_owned()))
        } else {
            Err(Refused::new(
                TreeRefusalReason::InvalidInstant,
                format!(
                    "--at {:?} is not an instant in UTC to the second, YYYY-MM-DDTHH:MM:SSZ",
                    cut(at)
                ),
            ))
        }
    }

    /// The instant now.
    ///
    /// # Errors
    /// `StoreUnreadable` when the host clock cannot be read.
    pub fn now(&self) -> Result<Timestamp, Refused> {
        match self {
            Self::Host => instant(&SystemClock)
                .map(|at| Timestamp(at.0))
                .map_err(|error| crate::unreadable("the host clock", error)),
            Self::At(at) => Ok(Timestamp(at.clone())),
        }
    }
}

/// At most 64 bytes of `text`, cut on a character boundary.
pub(crate) fn cut(text: &str) -> &str {
    let mut end = text.len().min(64);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
