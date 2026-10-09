//! The order a poll reads channels in.
//!
//! Each member channel is ranked by four keys, each deciding only where the ones before it tie:
//!
//! 1. its weight, highest first: the heaviest objective it serves, an entry of the configuration's
//!    `channels` naming it by id or by name (with or without `#`), each objective weighed by the
//!    poll's objectives; a channel no entry names, or whose objectives the poll does not weigh,
//!    weighs 0;
//! 2. its staleness, highest first: how long ago the newest message the plugin stored for it was
//!    posted, a channel with no stored `ts` stalest of all;
//! 3. its volume, highest first: its member count as the channel list reports it;
//! 4. a draw from the seed: a hash of the seed and the channel id, so one seed always orders the
//!    same channels the same way and another seed may order them otherwise.

use loom::plugin::{Cursor, Objective};
use loom::slack::SlackConfig;

use crate::poll::ts_micros;

/// A channel the bot is a member of, as the channel list reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Member {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    pub(crate) members: i64,
}

/// `channels` in walk order, at `now` (microseconds since the Unix epoch).
pub(crate) fn order(
    channels: Vec<Member>,
    config: &SlackConfig,
    objectives: &[Objective],
    cursors: &[Cursor],
    now: i64,
) -> Vec<Member> {
    let mut ranked: Vec<(f64, i64, i64, u64, Member)> = channels
        .into_iter()
        .map(|channel| {
            (
                weight(&channel, config, objectives),
                staleness(&channel, cursors, now),
                channel.members,
                draw(config.seed, &channel.id),
                channel,
            )
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then(b.2.cmp(&a.2))
            .then(a.3.cmp(&b.3))
            .then(a.4.id.cmp(&b.4.id))
    });
    ranked.into_iter().map(|ranked| ranked.4).collect()
}

/// The heaviest weight among the objectives `channel` serves; 0 when it serves none.
fn weight(channel: &Member, config: &SlackConfig, objectives: &[Objective]) -> f64 {
    let named = |entry: &str| {
        let entry = entry.trim_start_matches('#');
        entry == channel.id || channel.name.as_deref() == Some(entry)
    };
    config
        .channels
        .iter()
        .filter(|entry| named(&entry.channel))
        .flat_map(|entry| &entry.objectives)
        .filter_map(|served| {
            objectives
                .iter()
                .find(|objective| objective.name == *served)
                .and_then(|objective| objective.weight.0.trim().parse::<f64>().ok())
                .filter(|weight| weight.is_finite())
        })
        .fold(0.0, f64::max)
}

/// Microseconds since the newest message stored for `channel`; `i64::MAX` with none stored.
fn staleness(channel: &Member, cursors: &[Cursor], now: i64) -> i64 {
    cursors
        .iter()
        .find(|cursor| cursor.name == channel.id)
        .and_then(|cursor| ts_micros(&cursor.value))
        .map_or(i64::MAX, |stored| now.saturating_sub(stored).max(0))
}

/// The seed's draw for `id`: SplitMix64 over the seed mixed with the FNV-1a hash of the id.
fn draw(seed: i64, id: &str) -> u64 {
    let hash = id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let mut z = (seed as u64 ^ hash).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
