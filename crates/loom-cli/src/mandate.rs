//! `approve --mandate <FILE>` and `resume --mandate <FILE>` (`story:mandate-approval-on-resume`):
//! Mandate, configured from the host-trusted file `FILE` names (`loom.authority.MandateHost`), and
//! the state directory's approval event log (`intake.routing.MandateApprovalEntry`), as the
//! slice's [`MandateApprovals`].
//!
//! `approve --mandate` records a Mandate approval of the held request in the log: the approver is
//! the host file's, the expiry `approval_seconds` after the approval. `resume --mandate` clears the
//! held request through Mandate's `check_with_approval` with the approval the grant names, over
//! every entry of the log, and appends the `ApprovedDecisionRecorded` event it returns before the
//! request is performed, holding the state directory's lock throughout
//! ([`MandateApprovalLog::append_with`]). The flag and the log go when Mandate records approvals
//! itself.

use std::io::Read as _;
use std::path::Path;

use b10x_loom_commission::model::responsibility::{AuthorityContext, CommissionData};
use b10x_loom_intake_slice::grant::{
    self, GrantData, HeldApproval, MandateApprovalId, MandateApprovals,
};
use b10x_loom_intake_slice::mandate_log::MandateApprovalLog;
use intake_model::primitives::Timestamp;
use intake_model::routing::MandateApprovalEntry;
use loom_mandate::approval::{HeldRequest, HostedMandate};

use crate::regular_file;

/// The largest Mandate host file read, in bytes.
pub const MAX_HOST_BYTES: u64 = 1024 * 1024;

/// The Mandate host file at `path`, read and checked.
///
/// # Errors
/// Why it cannot be used, naming the file: it is no regular file, larger than [`MAX_HOST_BYTES`],
/// unreadable, or not a `loom.authority.MandateHost` `HostedMandate::parse` accepts.
pub fn read_host(path: &Path) -> Result<HostedMandate, String> {
    let named = |why: &dyn std::fmt::Display| format!("the Mandate file {}: {why}", path.display());
    let file = regular_file::open(path).map_err(|why| named(&why))?;
    let mut text = String::new();
    file.take(MAX_HOST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|error| named(&error))?;
    if text.len() as u64 > MAX_HOST_BYTES {
        return Err(named(&format!("is larger than {MAX_HOST_BYTES} bytes")));
    }
    HostedMandate::parse(&text).map_err(|why| named(&why))
}

/// Mandate as the slice asks it: the host file's Mandate over one state directory's log.
pub struct Mandate<'a> {
    hosted: HostedMandate,
    log: &'a MandateApprovalLog,
    clock: Box<dyn Fn() -> Timestamp + 'a>,
}

impl<'a> Mandate<'a> {
    /// Mandate configured by `hosted` over `log`, deciding at the instants `clock` reads.
    pub fn new(
        hosted: HostedMandate,
        log: &'a MandateApprovalLog,
        clock: impl Fn() -> Timestamp + 'a,
    ) -> Self {
        Self {
            hosted,
            log,
            clock: Box::new(clock),
        }
    }
}

impl MandateApprovals for Mandate<'_> {
    fn record(&self, held: &HeldApproval<'_>) -> Result<MandateApprovalId, String> {
        let id = MandateApprovalId(grant::fresh_id(
            "mandate-approval",
            &held.action_request_id.0.0,
        ));
        let expires_at = grant::after(held.at, self.hosted.approval_seconds())?;
        let approval = self
            .hosted
            .approval(
                &HeldRequest {
                    commission_id: held.commission_id,
                    action_request_id: held.action_request_id,
                    case_id: held.case_id,
                    capability: held.capability,
                },
                &id,
                held.at,
                &expires_at,
            )
            .map_err(|refused| refused.to_string())?;
        self.log
            .append(&MandateApprovalEntry::ApprovalRecorded(approval))
            .map_err(|error| error.to_string())?;
        Ok(id)
    }

    fn authority_context(&self) -> AuthorityContext {
        self.hosted.authority_context()
    }

    fn clear(
        &self,
        commission: &CommissionData,
        capability: &str,
        grant: &GrantData,
        approval: &MandateApprovalId,
    ) -> Result<(), String> {
        let at = (self.clock)();
        let decision = grant::fresh_id("mandate-decision", &approval.0.0);
        self.log
            .append_with(|entries| {
                self.hosted
                    .clear(
                        commission,
                        capability,
                        &grant.action_request_id,
                        approval,
                        entries,
                        &at,
                        &decision.0,
                    )
                    .map(MandateApprovalEntry::ApprovedDecisionRecorded)
            })
            .map_err(|error| format!("the Mandate approval log: {error}"))?
            .map(|_| ())
            .map_err(|refused| refused.to_string())
    }
}
