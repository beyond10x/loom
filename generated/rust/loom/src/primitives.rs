// generated from loom v1
// contract digest 37453704565c94aa7cfe18cd316f7df45e113ce53847f43614129aacb2640ee5
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

/// The three-valued reading every generated `broken_invariant` shares.
///
/// `Some(true)` holds, `Some(false)` is broken and `None` is unknown: a value the invariant reads
/// is absent, so it decides nothing. Numbers compare by their exact decimal value, a `Timestamp` by
/// the RFC 3339 instant it names, other text by its UTF-8 bytes, and `Bytes` as padded base64 —
/// the reading the conformance interpreter gives the same values.
pub mod invariant {
    use std::borrow::Cow;
    use std::cmp::Ordering;

    /// A number by its exact decimal value: `digits × 10^exponent`, with no leading or trailing
    /// zero in `digits`, and zero as no digits at all — so equal values are equal here.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Number {
        negative: bool,
        digits: String,
        exponent: i128,
    }

    impl Number {
        /// Reads a decimal spelling such as `-10.50` or `15e-1`, or `None` when it is not one.
        pub fn parse(text: &str) -> Option<Self> {
            let (negative, rest) = match text.as_bytes().first() {
                Some(b'-') => (true, &text[1..]),
                Some(b'+') => (false, &text[1..]),
                _ => (false, text),
            };
            let (mantissa, exponent) = match rest.find(['e', 'E']) {
                Some(at) => (&rest[..at], rest[at + 1..].parse::<i128>().ok()?),
                None => (rest, 0),
            };
            let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
            let digit = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
            if (whole.is_empty() && fraction.is_empty()) || !digit(whole) || !digit(fraction) {
                return None;
            }
            let all = format!("{whole}{fraction}");
            let leading = all.trim_start_matches('0');
            let significant = leading.trim_end_matches('0');
            if significant.is_empty() {
                return Some(Self { negative: false, digits: String::new(), exponent: 0 });
            }
            let dropped = i128::try_from(leading.len() - significant.len()).ok()?;
            let scale = i128::try_from(fraction.len()).ok()?;
            Some(Self {
                negative,
                digits: significant.to_owned(),
                exponent: exponent.checked_sub(scale)?.checked_add(dropped)?,
            })
        }

        fn is_zero(&self) -> bool {
            self.digits.is_empty()
        }

        fn sign(&self) -> i8 {
            if self.is_zero() {
                0
            } else if self.negative {
                -1
            } else {
                1
            }
        }

        /// Where the most significant digit sits.
        fn order(&self) -> i128 {
            i128::try_from(self.digits.len()).unwrap_or(i128::MAX).saturating_add(self.exponent)
        }
    }

    impl PartialOrd for Number {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Number {
        fn cmp(&self, other: &Self) -> Ordering {
            let sign = self.sign().cmp(&other.sign());
            if sign != Ordering::Equal || self.is_zero() {
                return sign;
            }
            let magnitude = self
                .order()
                .cmp(&other.order())
                .then_with(|| self.digits.as_str().cmp(other.digits.as_str()));
            if self.negative {
                magnitude.reverse()
            } else {
                magnitude
            }
        }
    }

    /// One value an invariant reads.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Fact<'a> {
        /// A Boolean.
        Bool(bool),
        /// A number.
        Number(Number),
        /// Text: a `String`, an enum variant's name, or the rendering of a `Timestamp`,
        /// `Duration`, `Uuid` or `Bytes`.
        Text(Cow<'a, str>),
    }

    impl<'a> Fact<'a> {
        /// An `Integer`.
        pub fn integer(value: i64) -> Self {
            Self::Number(Number::parse(&value.to_string()).unwrap_or(Number {
                negative: false,
                digits: String::new(),
                exponent: 0,
            }))
        }

        /// The number of elements, or of Unicode scalar values in a text.
        pub fn count(value: usize) -> Self {
            Self::integer(i64::try_from(value).unwrap_or(i64::MAX))
        }

        /// A `Decimal` or a number literal by its spelling; `None` when it spells no number.
        pub fn number(text: &str) -> Option<Self> {
            Number::parse(text).map(Self::Number)
        }

        /// Text.
        pub fn text(text: &'a str) -> Self {
            Self::Text(Cow::Borrowed(text))
        }

        /// `Bytes`, as the padded base64 the wire carries them in.
        pub fn bytes(bytes: &[u8]) -> Self {
            const ALPHABET: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
            for chunk in bytes.chunks(3) {
                let group = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
                let indices = [
                    group[0] >> 2,
                    ((group[0] & 0b11) << 4) | (group[1] >> 4),
                    ((group[1] & 0b1111) << 2) | (group[2] >> 6),
                    group[2] & 0b11_1111,
                ];
                for (position, index) in indices.iter().enumerate() {
                    if position <= chunk.len() {
                        out.push(char::from(ALPHABET[usize::from(*index)]));
                    } else {
                        out.push('=');
                    }
                }
            }
            Self::Text(Cow::Owned(out))
        }
    }

    /// A comparison operator.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Op {
        /// `==`
        Eq,
        /// `!=`
        Ne,
        /// `<`
        Lt,
        /// `<=`
        Le,
        /// `>`
        Gt,
        /// `>=`
        Ge,
    }

    impl Op {
        fn orders(self) -> bool {
            !matches!(self, Self::Eq | Self::Ne)
        }

        fn accepts(self, ordering: Ordering) -> bool {
            match self {
                Self::Eq => ordering == Ordering::Equal,
                Self::Ne => ordering != Ordering::Equal,
                Self::Lt => ordering == Ordering::Less,
                Self::Le => ordering != Ordering::Greater,
                Self::Gt => ordering == Ordering::Greater,
                Self::Ge => ordering != Ordering::Less,
            }
        }
    }

    /// A string operator.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TextOp {
        /// `starts_with`
        StartsWith,
        /// `ends_with`
        EndsWith,
        /// `contains`
        Contains,
    }

    /// Whether a value breaks the invariant: false, and not merely unknown.
    pub fn broken(truth: Option<bool>) -> bool {
        truth == Some(false)
    }

    /// Kleene conjunction: false dominates, then unknown.
    pub fn all<I: IntoIterator<Item = Option<bool>>>(truths: I) -> Option<bool> {
        let mut result = Some(true);
        for truth in truths {
            match truth {
                Some(false) => return Some(false),
                None => result = None,
                Some(true) => {}
            }
        }
        result
    }

    /// Kleene disjunction: true dominates, then unknown.
    pub fn any<I: IntoIterator<Item = Option<bool>>>(truths: I) -> Option<bool> {
        let mut result = Some(false);
        for truth in truths {
            match truth {
                Some(true) => return Some(true),
                None => result = None,
                Some(false) => {}
            }
        }
        result
    }

    /// Kleene negation: unknown stays unknown.
    pub fn not(truth: Option<bool>) -> Option<bool> {
        truth.map(|holds| !holds)
    }

    /// Every element satisfies `body`; an empty collection holds, an absent one is unknown.
    pub fn forall<I: IntoIterator>(
        items: Option<I>,
        body: impl FnMut(I::Item) -> Option<bool>,
    ) -> Option<bool> {
        all(items?.into_iter().map(body))
    }

    /// Some element satisfies `body`; an empty collection does not, an absent one is unknown.
    pub fn exists<I: IntoIterator>(
        items: Option<I>,
        body: impl FnMut(I::Item) -> Option<bool>,
    ) -> Option<bool> {
        any(items?.into_iter().map(body))
    }

    /// The fact is observed and truthy: `true`, a number other than zero, or text that is neither
    /// empty nor `false`.
    pub fn truthy(fact: Option<Fact<'_>>) -> Option<bool> {
        Some(match fact? {
            Fact::Bool(flag) => flag,
            Fact::Number(number) => !number.is_zero(),
            Fact::Text(text) => !text.is_empty() && text != "false",
        })
    }

    /// The observed fact equals one of `values`.
    pub fn any_of(fact: Option<Fact<'_>>, values: &[Option<Fact<'_>>]) -> Option<bool> {
        let fact = fact?;
        Some(values.iter().any(|value| value.as_ref() == Some(&fact)))
    }

    /// The observed fact equals none of `values`.
    pub fn none_of(fact: Option<Fact<'_>>, values: &[Option<Fact<'_>>]) -> Option<bool> {
        not(any_of(fact, values))
    }

    /// The observed text begins with, ends with or contains `literal`, byte for byte; any other
    /// observed value, or a literal that is not text, does not match.
    pub fn text_match(fact: Option<Fact<'_>>, op: TextOp, literal: Option<&str>) -> Option<bool> {
        let fact = fact?;
        Some(match (fact, literal) {
            (Fact::Text(text), Some(literal)) => match op {
                TextOp::StartsWith => text.as_bytes().starts_with(literal.as_bytes()),
                TextOp::EndsWith => text.as_bytes().ends_with(literal.as_bytes()),
                TextOp::Contains => text.contains(literal),
            },
            _ => false,
        })
    }

    /// The observed text equals one of `literals` under ASCII case folding.
    pub fn fold_match(fact: Option<Fact<'_>>, literals: &[&str]) -> Option<bool> {
        Some(match fact? {
            Fact::Text(text) => literals.iter().any(|literal| text.eq_ignore_ascii_case(literal)),
            _ => false,
        })
    }

    /// Compares two operands. `instant` says a fact operand is a declared `Timestamp`, ordered by
    /// the instant it names; `bytes` says every fact operand orders its text by UTF-8 bytes.
    pub fn compare(
        left: Option<Fact<'_>>,
        op: Op,
        right: Option<Fact<'_>>,
        instant: bool,
        bytes: bool,
    ) -> Option<bool> {
        let (left, right) = (left?, right?);
        if let (true, Fact::Text(left), Fact::Text(right)) = (instant, &left, &right) {
            if let (Some(left), Some(right)) = (rfc3339(left), rfc3339(right)) {
                return Some(op.accepts(left.cmp(&right)));
            }
        }
        match (&left, &right) {
            (Fact::Number(left), Fact::Number(right)) => Some(op.accepts(left.cmp(right))),
            (Fact::Text(left), Fact::Text(right)) if op.orders() => {
                (!instant && bytes).then(|| op.accepts(left.as_bytes().cmp(right.as_bytes())))
            }
            _ if op.orders() => None,
            _ => Some((left == right) == (op == Op::Eq)),
        }
    }

    /// The instant an RFC 3339 `date-time` names, as seconds from the epoch and nanoseconds.
    fn rfc3339(text: &str) -> Option<(i64, u32)> {
        let bytes = text.as_bytes();
        let digits = |from: usize, to: usize| -> Option<u32> {
            let slice = bytes.get(from..to)?;
            if slice.is_empty() || !slice.iter().all(u8::is_ascii_digit) {
                return None;
            }
            slice
                .iter()
                .try_fold(0u32, |total, digit| Some(total * 10 + u32::from(digit - b'0')))
        };
        let at = |index: usize, expected: &[u8]| bytes.get(index).is_some_and(|b| expected.contains(b));
        if !(at(4, b"-") && at(7, b"-") && at(10, b"Tt") && at(13, b":") && at(16, b":")) {
            return None;
        }
        let (year, month, day) = (i64::from(digits(0, 4)?), digits(5, 7)?, digits(8, 10)?);
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let length = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return None,
        };
        if day < 1 || day > length {
            return None;
        }
        let (hour, minute, second) = (digits(11, 13)?, digits(14, 16)?, digits(17, 19)?);
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        let mut position = 19;
        let mut nanos = 0u32;
        if at(position, b".") {
            let start = position + 1;
            let mut end = start;
            while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
            let width = end - start;
            if width == 0 || width > 9 {
                return None;
            }
            nanos = digits(start, end)? * 10u32.pow(u32::try_from(9 - width).ok()?);
            position = end;
        }
        let offset = match bytes.get(position..)? {
            b"Z" | b"z" => 0i64,
            [sign @ (b'+' | b'-'), _, _, b':', _, _] => {
                let (hours, minutes) = (digits(position + 1, position + 3)?, digits(position + 4, position + 6)?);
                if hours > 23 || minutes > 59 {
                    return None;
                }
                let magnitude = i64::from(hours * 3600 + minutes * 60);
                if *sign == b'-' {
                    -magnitude
                } else {
                    magnitude
                }
            }
            _ => return None,
        };
        let shifted = year - i64::from(month <= 2);
        let era = if shifted >= 0 { shifted } else { shifted - 399 } / 400;
        let year_of_era = shifted - era * 400;
        let month = i64::from(month);
        let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        let days = era * 146_097 + day_of_era - 719_468;
        Some((
            days * 86_400 + i64::from(hour * 3600 + minute * 60 + second) - offset,
            nanos,
        ))
    }
}
