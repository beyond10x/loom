//! How Loom shows a child's standard error: its last whole lines within [`STDERR_TAIL_BYTES`]
//! ([`stderr_tail`]), or every line of it ([`walked`]), each line that holds a credential's shape
//! withheld and never repeated ([`TOKEN_PREFIXES`], a URL with user information or a query
//! component, a PEM block, read over the whole text, so a block whose opening the tail's window
//! cut is withheld too), each terminal escape sequence and every other control character but a
//! tab removed and replaced, so what is shown is one inert text.
//!
//! One walk for every stderr Loom shows: the session executor's reason and failure record, and a
//! bound program's `ProgramError` refusal and run observation.

/// The most of a child's standard error a reason carries (a harness child's, a bound program's):
/// the last whole lines of it that fit, read from no more than this many bytes at its end.
pub const STDERR_TAIL_BYTES: usize = 4096;

/// The prefixes of the credentials a line of a child's standard error is withheld for,
/// matched in either case at the start of a word: the list `b10x_loom_agent::TOKEN_PREFIXES`
/// refuses in an agent spec, held equal to it by `crates/loom-agent/tests/token_prefixes.rs`.
pub const TOKEN_PREFIXES: [&str; 15] = [
    "xoxb-",
    "xoxp-",
    "xapp-",
    "sk-",
    "sk_live_",
    "sk_test_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "AKIA",
    "Bearer ",
    "-----BEGIN",
];

/// The last whole lines of `stderr`, a child's, that fit in [`STDERR_TAIL_BYTES`]: only the final
/// [`STDERR_TAIL_BYTES`] bytes are shown, a line they cut into is dropped whole, and so is a last
/// line longer than the bound, which a note stands in for. Every line that holds a credential's
/// shape (`credential_lines`) is withheld, read over the whole stderr, so a PEM block the window
/// cut from its opening is withheld as the whole walk ([`walked`]) withholds it; a note stands in
/// for each run of them, and every other control character but a tab is replaced, so the tail is
/// one inert text. Empty when the child said nothing.
#[must_use]
pub fn stderr_tail(stderr: &str) -> String {
    let stderr = stderr.trim_end();
    let mut start = stderr.len().saturating_sub(STDERR_TAIL_BYTES);
    while !stderr.is_char_boundary(start) {
        start += 1;
    }
    let from = if start == 0 || stderr.as_bytes()[start - 1] == b'\n' {
        start
    } else {
        match stderr[start..].find('\n') {
            Some(at) => start + at + 1,
            None => {
                return format!(
                    "[its last line, longer than {STDERR_TAIL_BYTES} bytes, is not shown]"
                );
            }
        }
    };
    // The window starts a line, so its lines are the whole stderr's past the ones before it.
    let before = stderr.as_bytes()[..from]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count();
    let (lines, withheld) = walk_lines(stderr);
    let shown = noted(lines.into_iter().skip(before), &withheld[before..]);
    // A note or a replaced character may be longer than what it stands in for: the bound holds
    // for the tail as shown, its first lines going first. A last line that no longer fits once
    // shown is dropped as one the window cannot hold, a note standing in for it.
    let mut kept = 0;
    let mut size = 0;
    for line in shown.iter().rev() {
        let more = line.len() + usize::from(kept > 0);
        if size + more > STDERR_TAIL_BYTES {
            break;
        }
        size += more;
        kept += 1;
    }
    if kept == 0 {
        return format!(
            "[its last line, longer than {STDERR_TAIL_BYTES} bytes as shown, is not shown]"
        );
    }
    shown[shown.len() - kept..].join("\n")
}

/// The lines of `text` as a tail or a record shows them: each one inert (`inert`), and each
/// that holds a credential's shape (`credential_lines`) withheld, read in four forms: with its
/// escape sequences and control characters removed, as a terminal shows it; with each replaced;
/// and both again with each escape sequence's intermediate and final bytes kept as text
/// (`kept`), so a sequence whose reading takes a token's first character hides no prefix. A
/// note stands in for each run of withheld lines. A line's `\r` ending is dropped.
#[must_use]
pub fn walked(text: &str) -> Vec<String> {
    let (lines, withheld) = walk_lines(text);
    noted(lines, &withheld)
}

/// The lines of `text`, each as shown ([`inert`]), and which of them [`walked`] withholds.
fn walk_lines(text: &str) -> (Vec<String>, Vec<bool>) {
    let raw: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let forms: Vec<(String, String)> = raw.iter().map(|line| inert(line)).collect();
    let kept_forms: Vec<(String, String)> = raw.iter().map(|line| kept(line)).collect();
    let pick = |forms: &[(String, String)], second: bool| -> Vec<bool> {
        let lines: Vec<&str> = forms
            .iter()
            .map(|(first, other)| if second { other } else { first }.as_str())
            .collect();
        credential_lines(&lines)
    };
    let withheld: Vec<bool> = pick(&forms, false)
        .into_iter()
        .zip(pick(&forms, true))
        .zip(pick(&kept_forms, false))
        .zip(pick(&kept_forms, true))
        .map(|(((shown, bare), marked), joined)| shown || bare || marked || joined)
        .collect();
    (
        forms.into_iter().map(|(shown, _)| shown).collect(),
        withheld,
    )
}

/// `lines` with each one `withheld` marks left out, a note standing in for each run of them.
fn noted(lines: impl IntoIterator<Item = String>, withheld: &[bool]) -> Vec<String> {
    let mut shown: Vec<String> = Vec::new();
    let mut run = 0usize;
    let note = |run: usize| match run {
        1 => "[a line withheld: it holds a credential's shape]".to_owned(),
        n => format!("[{n} lines withheld: they hold a credential's shape]"),
    };
    for (line, withheld) in lines.into_iter().zip(withheld.iter().copied()) {
        if withheld {
            run += 1;
            continue;
        }
        if run > 0 {
            shown.push(note(run));
            run = 0;
        }
        shown.push(line);
    }
    if run > 0 {
        shown.push(note(run));
    }
    shown
}

/// `line` read as [`inert`] reads it, twice, but each escape sequence's intermediate bytes
/// (`0x20..=0x2f`) and final byte kept as text, since the one the sequence ends with may be a
/// token's first character: marked, each sequence and each other control character but a tab
/// replaced by one U+FFFD before what it keeps, so a token joined to the word before it still
/// starts one; and joined, each of them removed, so a sequence inside a prefix hides it from
/// neither. A control sequence's parameter bytes (`0x30..=0x3f`, `?` among them) and a string's
/// body (`ESC ]`, `ESC P`, `ESC X`, `ESC ^`, `ESC _`, up to `BEL` or `ESC \`) are never kept.
fn kept(line: &str) -> (String, String) {
    let mut marked = String::with_capacity(line.len());
    let mut joined = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    let mut keep = |c: char, marked: &mut String| {
        marked.push(c);
        joined.push(c);
    };
    while let Some(c) = chars.next() {
        if !c.is_control() || c == '\t' {
            keep(c, &mut marked);
            continue;
        }
        marked.push(char::REPLACEMENT_CHARACTER);
        let csi = match c {
            '\u{9b}' => true,
            '\u{1b}' => match chars.peek() {
                Some('[') => {
                    chars.next();
                    true
                }
                Some(']' | 'P' | 'X' | '^' | '_') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                    continue;
                }
                Some(_) => false,
                None => continue,
            },
            _ => continue,
        };
        let final_bytes = if csi {
            while chars
                .next_if(|c| ('\u{30}'..='\u{3f}').contains(c))
                .is_some()
            {}
            '\u{40}'..='\u{7e}'
        } else {
            '\u{30}'..='\u{7e}'
        };
        while let Some(c) = chars.next_if(|c| ('\u{20}'..='\u{2f}').contains(c)) {
            keep(c, &mut marked);
        }
        if let Some(c) = chars.next_if(|c| final_bytes.contains(c)) {
            keep(c, &mut marked);
        }
    }
    (marked, joined)
}

/// `line` made inert, twice: as shown, each terminal escape sequence and each other control
/// character but a tab replaced by one U+FFFD, which no word continues across; and bare, each of
/// them removed, the text a terminal would show. An escape sequence is `ESC` with what it takes:
/// a control sequence (`ESC [` or `U+009B`, its parameter and intermediate bytes, its final
/// byte); a string (`ESC ]`, `ESC P`, `ESC X`, `ESC ^`, `ESC _`, up to `BEL` or `ESC \`); or
/// its intermediate bytes and one final character. One the line ends inside ends with it.
fn inert(line: &str) -> (String, String) {
    let mut shown = String::with_capacity(line.len());
    let mut bare = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_control() || c == '\t' {
            shown.push(c);
            bare.push(c);
            continue;
        }
        shown.push(char::REPLACEMENT_CHARACTER);
        let csi = match c {
            '\u{9b}' => true,
            '\u{1b}' => match chars.peek() {
                Some('[') => {
                    chars.next();
                    true
                }
                Some(']' | 'P' | 'X' | '^' | '_') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                    continue;
                }
                Some(_) => {
                    while chars
                        .next_if(|c| ('\u{20}'..='\u{2f}').contains(c))
                        .is_some()
                    {}
                    chars.next_if(|c| ('\u{30}'..='\u{7e}').contains(c));
                    continue;
                }
                None => false,
            },
            _ => false,
        };
        if csi {
            while chars
                .next_if(|c| ('\u{20}'..='\u{3f}').contains(c))
                .is_some()
            {}
            chars.next_if(|c| ('\u{40}'..='\u{7e}').contains(c));
        }
    }
    (shown, bare)
}

/// Which of `lines` hold a credential's shape: a URL with user information or a query
/// component, a word that starts with one of [`TOKEN_PREFIXES`] in either case (a word starting
/// where the line does or after a character that is neither a letter nor a digit), or a line of a
/// PEM block, from its `-----BEGIN` line to its `-----END` line; an `-----END` line whose
/// `-----BEGIN` the tail does not hold withholds every line before it too.
fn credential_lines(lines: &[&str]) -> Vec<bool> {
    let mut withheld: Vec<bool> = lines.iter().map(|line| holds_credential(line)).collect();
    let mut in_block = false;
    // Every line before `filled` is withheld already, so an `-----END` line without its
    // `-----BEGIN` fills only from there: a whole stderr of them is walked in one pass.
    let mut filled = 0;
    for at in 0..lines.len() {
        let lower = lines[at].to_ascii_lowercase();
        if lower.contains("-----begin") {
            in_block = true;
        }
        if in_block {
            withheld[at] = true;
        }
        if lower.contains("-----end") {
            if !in_block {
                withheld[filled..=at]
                    .iter_mut()
                    .for_each(|line| *line = true);
                filled = at + 1;
            }
            in_block = false;
        }
    }
    withheld
}

/// Whether `line` holds a credential's shape, as [`credential_lines`] reads one line.
fn holds_credential(line: &str) -> bool {
    // A tab is a space here, so `Bearer` followed by either is read alike; offsets are kept.
    let lower = line.to_ascii_lowercase().replace('\t', " ");
    let prefixed = TOKEN_PREFIXES.iter().any(|prefix| {
        let prefix = prefix.to_ascii_lowercase();
        lower.match_indices(&prefix).any(|(at, _)| {
            lower[..at]
                .chars()
                .next_back()
                .is_none_or(|before| !before.is_alphanumeric())
        })
    });
    prefixed || holds_credential_url(line)
}

/// Whether `line` holds a URL that carries user information or a query component. A URL is each
/// `://` with at least one scheme character before it (a letter, a digit, `+`, `-`, `.`) and
/// everything after it up to whitespace, so it lies in one word of the line. Its authority runs
/// from after `://` to the first `/`, or to the word's end: a `#` or a `?` does not end it, so a
/// password holding either is user information all the same. Each word is read in one pass: an
/// authority ends at the latest at the next `://`, whose `/` ends it, and a `?` after a `://` is
/// the word's last `?` after it, found once per word.
fn holds_credential_url(line: &str) -> bool {
    let is_scheme = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.');
    line.split(char::is_whitespace).any(|word| {
        let last_query = word.rfind('?');
        word.match_indices("://").any(|(at, _)| {
            if !word[..at].chars().next_back().is_some_and(is_scheme) {
                return false;
            }
            let rest = &word[at + 3..];
            last_query.is_some_and(|query| query >= at + 3)
                || rest[..rest.find('/').unwrap_or(rest.len())].contains('@')
        })
    })
}

/// Whether `url` carries user information or a query component, as the walk read it before
/// [`holds_credential_url`]: the equivalence its unit test holds it to.
#[cfg(test)]
fn url_holds_credential(url: &str) -> bool {
    let rest = &url[url.find("://").map_or(0, |at| at + 3)..];
    let authority = &rest[..rest.find('/').unwrap_or(rest.len())];
    authority.contains('@') || rest.contains('?')
}

/// Every URL `line` holds: each `://` with the scheme before it (letters, digits, `+`, `-`, `.`,
/// at least one) and everything after it up to whitespace; the reading before
/// [`holds_credential_url`], kept for its unit test.
#[cfg(test)]
fn line_urls(line: &str) -> Vec<&str> {
    let is_scheme = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.');
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find("://").map(|at| from + at) {
        let start = line[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_scheme(*c))
            .last()
            .map_or(at, |(index, _)| index);
        let end = line[at..]
            .find(char::is_whitespace)
            .map_or(line.len(), |end| at + end);
        if start < at {
            found.push(&line[start..end]);
        }
        from = at + 3;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_pem_end_without_its_begin_withholds_every_line_before_it() {
        let end = ["-----END ", "PRIVATE", " KEY-----"].concat();
        let lines = ["a", end.as_str(), "b", "c", end.as_str(), "d"];
        assert_eq!(
            credential_lines(&lines),
            [true, true, true, true, true, false]
        );
    }

    #[test]
    fn the_one_pass_url_reading_withholds_what_the_earlier_reading_withheld() {
        let pieces = [
            "a",
            "Z",
            "9",
            ":",
            "/",
            "//",
            "://",
            "@",
            "?",
            "#",
            " ",
            "\t",
            "+",
            "-",
            ".",
            "é",
            "\u{3000}",
            "https",
            "user:pw@",
            "h.example",
            "?q=1",
        ];
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = |bound: usize| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(seed >> 33).expect("31 bits fit") % bound
        };
        let mut held = 0;
        for _ in 0..20_000 {
            let line: String = (0..next(24)).map(|_| pieces[next(pieces.len())]).collect();
            let earlier = line_urls(&line).iter().any(|url| url_holds_credential(url));
            assert_eq!(holds_credential_url(&line), earlier, "{line:?}");
            held += usize::from(earlier);
        }
        assert!(
            held > 1_000,
            "the lines drawn hold credentials often enough: {held}"
        );
    }

    #[test]
    fn an_escape_sequence_is_one_replacement_as_shown_and_nothing_bare() {
        assert_eq!(
            inert("a\u{1b}[1;31mb\u{1b}]0;title\u{7}c\u{1b}(Bd\u{9b}2Ke\rf\tg\u{1b}"),
            (
                "a\u{fffd}b\u{fffd}c\u{fffd}d\u{fffd}e\u{fffd}f\tg\u{fffd}".to_owned(),
                "abcdef\tg".to_owned()
            )
        );
        assert!(holds_credential(&inert("key=\u{1b}[1msk-x").0));
        assert!(holds_credential(&inert("s\u{1b}[0mk-x").1));
    }

    #[test]
    fn an_escape_sequence_keeps_its_final_byte_and_no_parameter_or_string_body() {
        assert_eq!(
            kept("a\u{1b}gb\u{1b}[?25hc\u{1b}]8;;u?q\u{1b}\\d\u{1b}(Be\u{9b}1Kf\u{7}g\tz\u{1b}"),
            (
                "a\u{fffd}gb\u{fffd}hc\u{fffd}d\u{fffd}(Be\u{fffd}Kf\u{fffd}g\tz\u{fffd}"
                    .to_owned(),
                "agbhcd(BeKfg\tz".to_owned()
            )
        );
        let withheld = |line: &str| walked(line) != [line.to_owned()];
        for line in [
            "value=\u{1b}ghp_x",
            "value=x\u{1b}ghp_x",
            "value=\u{1b}[1ghp_x",
            "value=gh\u{1b}p_x",
            "\u{1b}-----BEGIN PRIVATE KEY-----",
        ] {
            assert!(withheld(line), "{line:?} is withheld");
        }
        for line in [
            "fetched https://h.example/a",
            "see https://h.example/a for more",
        ] {
            assert!(!withheld(line), "{line:?} is kept");
        }
        assert_eq!(
            walked("fetched https://h.example/a\u{1b}[?25h"),
            ["fetched https://h.example/a\u{fffd}"]
        );
        assert_eq!(
            walked("see \u{1b}]8;;https://h.example/e?q=7\u{1b}\\guide\u{1b}]8;;\u{1b}\\ now"),
            ["see \u{fffd}guide\u{fffd} now"]
        );
    }
}
