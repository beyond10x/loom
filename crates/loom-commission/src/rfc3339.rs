//! The instant an RFC 3339 date-time names, for a run budget's `deadline` and the trusted clock's
//! reading it is compared with (`story:run-budget-tokens-and-deadline`).
//!
//! It reads RFC 3339's `date-time` alone: a full date, `T` (or `t`), a full time with an optional
//! fraction of a second, then `Z` (or `z`) or a numeric offset `+hh:mm` or `-hh:mm`. A month's
//! days follow the Gregorian calendar, leap years included; a second of 60 (a leap second) is
//! read as the first second of the next minute. Anything else names no instant.

/// The instant `text` names, as whole seconds since 1970-01-01T00:00:00Z and the nanoseconds past
/// that second (`0..1_000_000_000`), or `None` when `text` is not an RFC 3339 date-time. A fraction
/// finer than a nanosecond is read and dropped.
pub(crate) fn instant(text: &str) -> Option<(i64, u32)> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let year = digits(bytes, &mut at, 4)?;
    literal(bytes, &mut at, b"-")?;
    let month = digits(bytes, &mut at, 2)?;
    literal(bytes, &mut at, b"-")?;
    let day = digits(bytes, &mut at, 2)?;
    literal(bytes, &mut at, b"Tt")?;
    let hour = digits(bytes, &mut at, 2)?;
    literal(bytes, &mut at, b":")?;
    let minute = digits(bytes, &mut at, 2)?;
    literal(bytes, &mut at, b":")?;
    let second = digits(bytes, &mut at, 2)?;
    let nanos = fraction(bytes, &mut at)?;
    let offset = offset(bytes, &mut at)?;
    if at != bytes.len() {
        return None;
    }
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second - offset;
    Some((seconds, nanos))
}

/// Reads exactly `count` ASCII digits at `at` as a number.
fn digits(bytes: &[u8], at: &mut usize, count: usize) -> Option<i64> {
    let taken = bytes.get(*at..*at + count)?;
    let mut value = 0;
    for byte in taken {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + i64::from(byte - b'0');
    }
    *at += count;
    Some(value)
}

/// Reads one byte at `at`, which must be one of `allowed`.
fn literal(bytes: &[u8], at: &mut usize, allowed: &[u8]) -> Option<()> {
    let byte = bytes.get(*at)?;
    if !allowed.contains(byte) {
        return None;
    }
    *at += 1;
    Some(())
}

/// Reads an optional fraction of a second, `.` and one digit or more, as nanoseconds.
fn fraction(bytes: &[u8], at: &mut usize) -> Option<u32> {
    if bytes.get(*at) != Some(&b'.') {
        return Some(0);
    }
    *at += 1;
    let start = *at;
    let mut nanos: u32 = 0;
    let mut scale: u32 = 100_000_000;
    while let Some(byte) = bytes.get(*at).filter(|byte| byte.is_ascii_digit()) {
        nanos += u32::from(byte - b'0') * scale;
        scale /= 10;
        *at += 1;
    }
    (*at > start).then_some(nanos)
}

/// Reads the offset, `Z`, `z`, `+hh:mm` or `-hh:mm`, as the seconds the local time is ahead of UTC.
fn offset(bytes: &[u8], at: &mut usize) -> Option<i64> {
    let sign = match bytes.get(*at)? {
        b'Z' | b'z' => {
            *at += 1;
            return Some(0);
        }
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    *at += 1;
    let hours = digits(bytes, at, 2)?;
    literal(bytes, at, b":")?;
    let minutes = digits(bytes, at, 2)?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(sign * (hours * 3_600 + minutes * 60))
}

/// The days of `month` in `year`.
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The days from 1970-01-01 to the Gregorian date given, negative before it.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::instant;

    #[test]
    fn the_epoch_and_a_known_instant() {
        assert_eq!(instant("1970-01-01T00:00:00Z"), Some((0, 0)));
        assert_eq!(instant("2026-10-21T09:30:00Z"), Some((1_792_575_000, 0)));
        assert_eq!(instant("1969-12-31T23:59:59Z"), Some((-1, 0)));
        assert_eq!(instant("0000-01-01T00:00:00Z"), Some((-62_167_219_200, 0)));
    }

    #[test]
    fn leap_year_days() {
        assert_eq!(instant("2024-02-29T00:00:00Z"), Some((1_709_164_800, 0)));
        assert_eq!(instant("2000-02-29T00:00:00Z"), Some((951_782_400, 0)));
        assert_eq!(instant("2000-03-01T00:00:00Z"), Some((951_868_800, 0)));
        assert_eq!(instant("2023-02-29T00:00:00Z"), None);
        assert_eq!(instant("1900-02-29T00:00:00Z"), None);
        assert_eq!(instant("2023-02-28T23:59:59Z"), Some((1_677_628_799, 0)));
        assert_eq!(instant("2024-12-31T00:00:00Z"), Some((1_735_603_200, 0)));
    }

    #[test]
    fn a_month_has_its_own_days() {
        assert_eq!(instant("2026-04-31T00:00:00Z"), None);
        assert_eq!(instant("2026-01-31T00:00:00Z"), Some((1_769_817_600, 0)));
        assert_eq!(instant("2026-13-01T00:00:00Z"), None);
        assert_eq!(instant("2026-00-01T00:00:00Z"), None);
        assert_eq!(instant("2026-01-00T00:00:00Z"), None);
    }

    #[test]
    fn offsets_name_the_same_instant() {
        let utc = instant("2026-10-21T09:30:00Z");
        assert_eq!(instant("2026-10-21T11:30:00+02:00"), utc);
        assert_eq!(instant("2026-10-21T04:00:00-05:30"), utc);
        assert_eq!(instant("2026-10-21T09:30:00+00:00"), utc);
        assert_eq!(instant("2026-10-21T09:30:00-00:00"), utc);
        assert_eq!(instant("2026-10-22T00:30:00+15:00"), utc);
        assert_eq!(instant("2026-10-21t09:30:00z"), utc);
        assert_eq!(instant("2026-10-21T09:30:00+24:00"), None);
        assert_eq!(instant("2026-10-21T09:30:00+02:60"), None);
        assert_eq!(instant("2026-10-21T09:30:00+0200"), None);
    }

    #[test]
    fn a_fraction_is_kept_to_the_nanosecond() {
        assert_eq!(
            instant("2026-10-21T09:30:00.5Z"),
            Some((1_792_575_000, 500_000_000))
        );
        assert_eq!(
            instant("2026-10-21T09:30:00.123456789123Z"),
            Some((1_792_575_000, 123_456_789))
        );
        assert_eq!(instant("1969-12-31T23:59:59.25Z"), Some((-1, 250_000_000)));
        assert_eq!(instant("2026-10-21T09:30:00.Z"), None);
    }

    #[test]
    fn a_leap_second_is_the_next_minutes_first() {
        assert_eq!(
            instant("2016-12-31T23:59:60Z"),
            instant("2017-01-01T00:00:00Z")
        );
        assert_eq!(instant("2026-10-21T09:30:61Z"), None);
    }

    #[test]
    fn anything_else_names_no_instant() {
        for text in [
            "",
            "next tuesday",
            "2026-10-21",
            "2026-10-21T09:30:00",
            "2026-10-21 09:30:00Z",
            "2026-10-21T24:00:00Z",
            "2026-10-21T09:60:00Z",
            "2026-1-21T09:30:00Z",
            "+2026-10-21T09:30:00Z",
            "2026-10-21T09:30:00Z ",
            "2026-10-21T09:30:00ZZ",
            "２026-10-21T09:30:00Z",
        ] {
            assert_eq!(instant(text), None, "{text:?}");
        }
    }
}
