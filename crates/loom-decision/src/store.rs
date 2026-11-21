//! The `DecisionStore` port and its in-memory implementation.

use std::fmt;

use loom::decision::{
    DecisionRequestId, DecisionRequestSnapshot, DecisionRequestState, EscalationSuppressedRecord,
    NoSuchRequest,
};

/// A store that could not read or write: where, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreFailure {
    /// Where the store failed, such as the operation and the request it addressed.
    pub at: String,
    /// Why.
    pub why: String,
}

impl fmt::Display for StoreFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the decision store failed at {}: {}", self.at, self.why)
    }
}

impl std::error::Error for StoreFailure {}

/// A write refused because the request is no longer in the state it was read in: the request
/// id, the state read (absent for a new request) and the state found (absent when none is
/// stored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConflict {
    /// The request written.
    pub request_id: DecisionRequestId,
    /// The request as it was read, or `None` for a new one.
    pub read: Option<Box<DecisionRequestSnapshot>>,
    /// The request the store holds, or `None` when it holds none.
    pub found: Option<Box<DecisionRequestSnapshot>>,
}

fn state(snapshot: Option<&DecisionRequestSnapshot>) -> String {
    snapshot.map_or_else(|| "absent".to_owned(), |held| format!("{:?}", held.state))
}

impl fmt::Display for StoreConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "decision request {} changed since it was read: read {}, found {}",
            self.request_id.0,
            state(self.read.as_deref()),
            state(self.found.as_deref())
        )
    }
}

impl std::error::Error for StoreConflict {}

/// Why [`DecisionStore::write`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// The request changed since it was read.
    Conflict(StoreConflict),
    /// The store could not write.
    Failure(StoreFailure),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict(conflict) => conflict.fmt(f),
            Self::Failure(failure) => failure.fmt(f),
        }
    }
}

impl std::error::Error for WriteError {}

/// Why [`DecisionStore::append_record`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendError {
    /// No request has the record's request id.
    NoSuchRequest(NoSuchRequest),
    /// The store could not write.
    Failure(StoreFailure),
}

impl fmt::Display for AppendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchRequest(missing) => {
                write!(f, "no decision request has the id {}", missing.request_id.0)
            }
            Self::Failure(failure) => failure.fmt(f),
        }
    }
}

impl std::error::Error for AppendError {}

/// Where decision requests, and the records kept beside them, are stored.
pub trait DecisionStore {
    /// The request with this id, or `None` where none is stored.
    ///
    /// # Errors
    ///
    /// [`StoreFailure`] when the store cannot read.
    fn read(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Option<DecisionRequestSnapshot>, StoreFailure>;

    /// Stores `next` under its request id when the store still holds `read` there (`None`: no
    /// request), replacing it.
    ///
    /// # Errors
    ///
    /// [`WriteError::Conflict`] when the store holds anything else, a new request whose id was
    /// stored since the read included, and nothing is written; [`WriteError::Failure`] when the
    /// store cannot write.
    fn write(
        &mut self,
        read: Option<&DecisionRequestSnapshot>,
        next: DecisionRequestSnapshot,
    ) -> Result<(), WriteError>;

    /// Every stored request, in the order they were first stored.
    ///
    /// # Errors
    ///
    /// [`StoreFailure`] when the store cannot read.
    fn list(&self) -> Result<Vec<DecisionRequestSnapshot>, StoreFailure>;

    /// The Open or Escalated request whose `record_key` is `record_key`, if any.
    ///
    /// # Errors
    ///
    /// [`StoreFailure`] when the store cannot read.
    fn open_for_record(&self, record_key: &str) -> Result<Option<DecisionRequestId>, StoreFailure> {
        Ok(self
            .list()?
            .into_iter()
            .find(|held| {
                matches!(
                    held.state,
                    DecisionRequestState::Open | DecisionRequestState::Escalated
                ) && held.data.record_key == record_key
            })
            .map(|held| held.data.request_id))
    }

    /// Keeps `record` beside the request it names, after the records kept before it.
    ///
    /// # Errors
    ///
    /// [`AppendError::NoSuchRequest`] when no request has its request id, and nothing is kept;
    /// [`AppendError::Failure`] when the store cannot write.
    fn append_record(&mut self, record: EscalationSuppressedRecord) -> Result<(), AppendError>;

    /// The records kept beside the request, in the order they were appended; empty for a request
    /// with none.
    ///
    /// # Errors
    ///
    /// [`StoreFailure`] when the store cannot read.
    fn records(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Vec<EscalationSuppressedRecord>, StoreFailure>;
}

/// A [`DecisionStore`] in memory: the requests in the order first stored, and the records
/// appended beside them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryDecisionStore {
    requests: Vec<DecisionRequestSnapshot>,
    records: Vec<EscalationSuppressedRecord>,
}

impl InMemoryDecisionStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A store holding `requests`, as a store reopened over what it held would, and no records.
    #[must_use]
    pub fn with_requests(requests: Vec<DecisionRequestSnapshot>) -> Self {
        Self {
            requests,
            records: Vec::new(),
        }
    }

    fn held(&self, request_id: &DecisionRequestId) -> Option<&DecisionRequestSnapshot> {
        self.requests
            .iter()
            .find(|held| &held.data.request_id == request_id)
    }
}

impl DecisionStore for InMemoryDecisionStore {
    fn read(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Option<DecisionRequestSnapshot>, StoreFailure> {
        Ok(self.held(request_id).cloned())
    }

    fn write(
        &mut self,
        read: Option<&DecisionRequestSnapshot>,
        next: DecisionRequestSnapshot,
    ) -> Result<(), WriteError> {
        let found = self.held(&next.data.request_id);
        if found != read {
            return Err(WriteError::Conflict(StoreConflict {
                request_id: next.data.request_id.clone(),
                read: read.cloned().map(Box::new),
                found: found.cloned().map(Box::new),
            }));
        }
        match self
            .requests
            .iter_mut()
            .find(|held| held.data.request_id == next.data.request_id)
        {
            Some(held) => *held = next,
            None => self.requests.push(next),
        }
        Ok(())
    }

    fn list(&self) -> Result<Vec<DecisionRequestSnapshot>, StoreFailure> {
        Ok(self.requests.clone())
    }

    fn append_record(&mut self, record: EscalationSuppressedRecord) -> Result<(), AppendError> {
        if self.held(&record.request_id).is_none() {
            return Err(AppendError::NoSuchRequest(NoSuchRequest {
                request_id: record.request_id,
            }));
        }
        self.records.push(record);
        Ok(())
    }

    fn records(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Vec<EscalationSuppressedRecord>, StoreFailure> {
        Ok(self
            .records
            .iter()
            .filter(|record| &record.request_id == request_id)
            .cloned()
            .collect())
    }
}
