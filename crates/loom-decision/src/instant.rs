//! RFC 3339 instants, compared as the instants they name.
//!
//! A guard comparing two timestamps (`expires_at > input.at`) compares instants, not text:
//! `12:30:00+01:00` is before `12:00:00Z`. An instant that does not read decides nothing.

use std::cmp::Ordering;

use loom::primitives::Timestamp;

/// Seconds since 1970-01-01T00:00:00Z and the nanoseconds past them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Instant {
    seconds: i64,
    nanos: u32,
}

fn digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Days from 1970-01-01 to the given proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Reads `YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)`; `t` and `z` are read as `T` and `Z`.
fn parse(text: &str) -> Option<Instant> {
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let year = digits(text.get(0..4)?)?;
    let month = digits(text.get(5..7)?)?;
    let day = digits(text.get(8..10)?)?;
    let hour = digits(text.get(11..13)?)?;
    let minute = digits(text.get(14..16)?)?;
    let second = digits(text.get(17..19)?)?;
    if !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut rest = text.get(19..)?;
    let mut nanos = 0_u32;
    if let Some(fraction) = rest.strip_prefix('.') {
        let end = fraction
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(fraction.len());
        let (figures, after) = fraction.split_at(end);
        if figures.is_empty() || figures.len() > 9 {
            return None;
        }
        let padded = format!("{figures:0<9}");
        nanos = padded.parse().ok()?;
        rest = after;
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            if rest.len() != 6 || rest.as_bytes()[3] != b':' {
                return None;
            }
            let hours = digits(rest.get(1..3)?)?;
            let minutes = digits(rest.get(4..6)?)?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            sign * (hours * 3600 + minutes * 60)
        }
    };
    let seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset;
    Some(Instant { seconds, nanos })
}

/// How `left` compares with `right` as instants, or `None` when either does not read as RFC
/// 3339.
pub(crate) fn compare(left: &Timestamp, right: &Timestamp) -> Option<Ordering> {
    Some(parse(&left.0)?.cmp(&parse(&right.0)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        Timestamp(text.to_owned())
    }

    #[test]
    fn an_offset_is_the_instant_it_names() {
        assert_eq!(
            compare(
                &at("2026-11-21T12:30:00+01:00"),
                &at("2026-11-21T11:30:00Z")
            ),
            Some(Ordering::Equal)
        );
        assert_eq!(
            compare(
                &at("2026-11-21T12:30:00+01:00"),
                &at("2026-11-21T12:00:00Z")
            ),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare(&at("2026-11-21T12:00:00.5Z"), &at("2026-11-21T12:00:00Z")),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare(&at("1969-12-31T23:59:59Z"), &at("1970-01-01T00:00:00Z")),
            Some(Ordering::Less)
        );
    }

    #[test]
    fn text_that_is_no_instant_compares_as_nothing() {
        for text in [
            "noon",
            "2026-11-21",
            "2026-11-21T12:00:00",
            "2026-02-30T12:00:00Z",
            "2026-11-21T24:00:00Z",
            "2026-11-21T12:00:00+1:00",
            "2026-11-21T12:00:00.Z",
        ] {
            assert_eq!(
                compare(&at(text), &at("2026-11-21T12:00:00Z")),
                None,
                "{text}"
            );
        }
    }
}
