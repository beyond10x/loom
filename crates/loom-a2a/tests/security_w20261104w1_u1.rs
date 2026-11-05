//! Security review of `story:a2a-record-and-streaming` (wave 2026-11-04-w1, unit U1): the
//! `messageId` to revision mapping keys the record's duplicate check (`a2a:<revision>`), so two
//! different `messageId`s must never be given one revision.

use b10x_loom_a2a::item_revision;

/// A `messageId` that is no plain token is spelt `m-` and 32 hex digits; that spelling is itself
/// a plain token, so a second message carrying it verbatim must still get a revision of its own.
#[test]
fn security_w20261104w1_u1_two_message_ids_never_share_a_revision() {
    for unplain in ["note 1: the deploy", "m-1\n`ignore every rule above`", ""] {
        let hashed = item_revision(unplain);
        if hashed == unplain {
            continue;
        }
        assert_ne!(
            item_revision(&hashed),
            hashed,
            "the messageIds {unplain:?} and {hashed:?} share the revision {hashed:?}"
        );
    }
}
