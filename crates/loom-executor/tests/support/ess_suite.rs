//! The committed conformance suite of `ess/` and the digest that ties it to its sources.
//!
//! `ess verify conform synthesize --suite-format 5` over `ess/` takes minutes since the decision
//! domain joined it, so it runs once, in `task suite-generate`, which writes [`SUITE`] and
//! [`RECORD`]. The record holds the digest of its inputs ([`inputs_digest`]: every file under
//! `ess/` by sorted path, and the version line of the `ess` that runs), the digest of the suite
//! file and synthesize's standard output. `task ess-gate` step 3 and `task conform` read the
//! committed suite after [`committed`] found both digests equal to the tree's; `task slow`
//! synthesizes again and compares bytes ([`compare_fresh`]).
//!
//! Standard library only: `tests/ess_gate.rs` includes this file and runs in std-only replicas of
//! the repository; `loom-xtask` and `b10x-loom-conformance`'s tests include it by `#[path]`.
//!
//! Which `ess` runs: every call carries `ESS_TOOLCHAIN` ([`Ess`]), the caller's or else the release
//! the repository's `ess/ess-inputs.yaml` requires, so a dispatcher newer than the pin runs the
//! pinned release; a plain release of the pin accepts the variable. The version the checks digest
//! is that of the `ess` step 1 holds to the pin with `--strict-requires`.

#![allow(dead_code)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

/// The committed suite, relative to the repository root.
pub const SUITE: &str = "generated/ess/loom/suite.json";
/// The committed record of the synthesis that wrote [`SUITE`], relative to the repository root.
pub const RECORD: &str = "generated/ess/loom/synthesis.txt";
/// The command that rewrites [`SUITE`] and [`RECORD`].
pub const REGENERATE: &str = "task suite-generate";
/// The first line of [`RECORD`].
pub const RECORD_FORMAT: &str = "format: loom-ess-synthesis/1";

/// What one `ess verify conform synthesize` run returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Synthesis {
    /// Exit code, `None` when a signal ended the run.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// The suite file it wrote, when it wrote one.
    pub suite: Option<Vec<u8>>,
}

impl Synthesis {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// The run as a failure prints it.
    pub fn describe(&self) -> String {
        format!(
            "exit status: {}; stdout: {}; stderr: {}",
            self.code
                .map_or_else(|| "signal".to_owned(), |code| code.to_string()),
            self.stdout.trim(),
            self.stderr.trim()
        )
    }
}

/// What [`RECORD`] says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The version line of the `ess` that synthesized.
    pub ess: String,
    /// [`inputs_digest`] of `ess/` when it synthesized.
    pub inputs: String,
    /// Synthesize's exit code, or `signal`.
    pub exit: String,
    /// [`digest`] of [`SUITE`], or [`NO_SUITE`] when synthesize wrote none.
    pub suite: String,
    /// Synthesize's standard output, verbatim.
    pub stdout: String,
}

/// What [`Record::suite`] says when synthesize wrote no suite.
pub const NO_SUITE: &str = "none";

impl Record {
    pub fn render(&self) -> String {
        format!(
            "{RECORD_FORMAT}\n\
             command: ess verify conform synthesize --path ess --scenarios ess --suite-format 5\n\
             regenerate: {REGENERATE}\n\
             ess: {}\n\
             inputs: {}\n\
             exit: {}\n\
             suite: {}\n\
             stdout:\n{}",
            self.ess, self.inputs, self.exit, self.suite, self.stdout
        )
    }

    /// Synthesize exited 0.
    pub fn success(&self) -> bool {
        self.exit == "0"
    }

    /// The synthesis as a failure prints it.
    pub fn describe(&self) -> String {
        format!(
            "the synthesis {RECORD} records: exit status: {}; stdout: {}",
            self.exit,
            self.stdout.trim()
        )
    }

    pub fn parse(text: &str) -> Result<Record, String> {
        let (head, stdout) = text
            .split_once("\nstdout:\n")
            .ok_or_else(|| "no `stdout:` line".to_owned())?;
        let mut lines = head.lines();
        if lines.next() != Some(RECORD_FORMAT) {
            return Err(format!("the first line is not `{RECORD_FORMAT}`"));
        }
        let mut field = |name: &str| -> Result<String, String> {
            let prefix = format!("{name}: ");
            lines
                .by_ref()
                .find_map(|line| line.strip_prefix(&prefix))
                .map(str::to_owned)
                .ok_or_else(|| format!("no `{name}:` line"))
        };
        let ess = field("ess")?;
        let inputs = field("inputs")?;
        let exit = field("exit")?;
        let suite = field("suite")?;
        Ok(Record {
            ess,
            inputs,
            exit,
            suite,
            stdout: stdout.to_owned(),
        })
    }
}

/// The `ess` a check runs, and the release it asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ess {
    pub program: PathBuf,
    /// The `ESS_TOOLCHAIN` each call carries: the caller's when it set one, else the release the
    /// repository's `ess/ess-inputs.yaml` requires ([`pinned_release`]), so a dispatcher newer than
    /// the pin runs the pinned release; `None` when neither names one.
    pub toolchain: Option<String>,
}

impl Ess {
    /// `program` for the repository at `root`.
    pub fn new(program: impl Into<PathBuf>, root: &Path) -> Ess {
        let toolchain = std::env::var("ESS_TOOLCHAIN")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| pinned_release(root));
        Ess {
            program: program.into(),
            toolchain,
        }
    }

    /// The program's path, as a message prints it.
    pub fn display(&self) -> std::path::Display<'_> {
        self.program.display()
    }

    /// A command that starts `ess`, with `ESS_TOOLCHAIN` set.
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(toolchain) = &self.toolchain {
            command.env("ESS_TOOLCHAIN", toolchain);
        }
        command
    }
}

/// The release `<root>/ess/ess-inputs.yaml` requires: `X.Y.Z` of its line `requires: ess X.Y.Z`.
pub fn pinned_release(root: &Path) -> Option<String> {
    let manifest = fs::read_to_string(root.join("ess/ess-inputs.yaml")).ok()?;
    manifest.lines().find_map(|line| {
        line.strip_prefix("requires: ess ")
            .map(|version| version.trim().to_owned())
            .filter(|version| !version.is_empty())
    })
}

/// `ess` on `PATH` for the repository at `root`.
pub fn ess_program(root: &Path) -> Ess {
    Ess::new("ess", root)
}

fn command(ess: &Ess, cwd: Option<&Path>) -> Command {
    let mut command = ess.command();
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
}

/// The version line of the `ess` that runs: the last line `ess --version` prints, which is the
/// delegate's when a dispatcher hands the call to a pinned release.
pub fn ess_version(ess: &Ess, cwd: Option<&Path>) -> Result<String, String> {
    let output = command(ess, cwd)
        .arg("--version")
        .output()
        .map_err(|e| format!("run `{} --version` (is ess on PATH?): {e}", ess.display()))?;
    if !output.status.success() {
        return Err(format!(
            "`{} --version` exited {}: {}",
            ess.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("`{} --version` printed nothing", ess.display()))
}

/// [`digest`] of the line `ess: <version>` and then every file under `dir`, in the byte order of
/// its `/`-separated path relative to `dir`, each as `<path>\n<length>\n<bytes>\n`.
pub fn inputs_digest(dir: &Path, ess_version: &str) -> Result<String, String> {
    let mut files = Vec::new();
    collect(dir, dir, &mut files)?;
    files.sort();
    let mut stream = format!("loom-ess-inputs/1\ness: {ess_version}\n").into_bytes();
    for relative in files {
        let path = dir.join(&relative);
        let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        stream.extend_from_slice(format!("{relative}\n{}\n", bytes.len()).as_bytes());
        stream.extend_from_slice(&bytes);
        stream.push(b'\n');
    }
    Ok(digest(&stream))
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| format!("stat {}: {e}", path.display()))?;
        if kind.is_dir() {
            collect(base, &path, out)?;
        } else {
            let relative = path
                .strip_prefix(base)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push(relative);
        }
    }
    Ok(())
}

/// Runs `ess verify conform synthesize --path <spec> --scenarios <spec> --suite-format 5 --out
/// <out>` and reads the suite it wrote.
pub fn synthesize(
    ess: &Ess,
    cwd: Option<&Path>,
    spec: &Path,
    out: &Path,
) -> Result<Synthesis, String> {
    if out.exists() {
        fs::remove_file(out).map_err(|e| format!("remove {}: {e}", out.display()))?;
    }
    let output = command(ess, cwd)
        .args(["verify", "conform", "synthesize", "--path"])
        .arg(spec)
        .arg("--scenarios")
        .arg(spec)
        .args(["--suite-format", "5", "--out"])
        .arg(out)
        .output()
        .map_err(|e| {
            format!(
                "run `{} verify conform synthesize` (is ess on PATH?): {e}",
                ess.display()
            )
        })?;
    Ok(Synthesis {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        suite: fs::read(out).ok(),
    })
}

/// [`synthesize`] of `spec` with the `ess` on `PATH`, made once per distinct input for every
/// process that shares `cache`: the result is kept under `cache` by [`inputs_digest`] of `spec`,
/// and a caller with an input already synthesized, or being synthesized, reads that result.
pub fn synthesize_once(ess: &Ess, spec: &Path, cache: &Path) -> Result<Synthesis, String> {
    let version = ess_version(ess, None)?;
    let key = inputs_digest(spec, &version)?;
    let key = key.trim_start_matches("sha256:").to_owned();
    fs::create_dir_all(cache).map_err(|e| format!("create {}: {e}", cache.display()))?;
    let done = cache.join(format!("{key}.done"));
    let lock = cache.join(format!("{key}.lock"));
    let started = Instant::now();
    loop {
        if let Some(found) = read_cached(cache, &key, &done)? {
            return Ok(found);
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
        {
            Ok(mut file) => {
                use std::io::Write as _;
                let _ = write!(file, "{}", std::process::id());
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if lock_is_stale(&lock) {
                    let _ = fs::remove_file(&lock);
                    continue;
                }
                if started.elapsed() > Duration::from_secs(900) {
                    return Err(format!(
                        "{} held for 900 s: remove it if no synthesis runs",
                        lock.display()
                    ));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("create {}: {e}", lock.display())),
        }
    }
    let result = synthesize(ess, None, spec, &cache.join(format!("{key}.suite.json")));
    let written = result.as_ref().map_err(Clone::clone).and_then(|synthesis| {
        let code = synthesis
            .code
            .map_or_else(|| "signal".to_owned(), |c| c.to_string());
        fs::write(cache.join(format!("{key}.stdout")), &synthesis.stdout)
            .and_then(|()| fs::write(cache.join(format!("{key}.stderr")), &synthesis.stderr))
            .and_then(|()| fs::write(cache.join(format!("{key}.code")), code))
            .and_then(|()| fs::write(&done, b""))
            .map_err(|e| format!("write under {}: {e}", cache.display()))
    });
    let _ = fs::remove_file(&lock);
    written?;
    result
}

/// A lock whose holder is gone: a process killed while it synthesized (a test past its time)
/// leaves its lock behind. Where `/proc` is absent, a lock older than 900 s.
fn lock_is_stale(lock: &Path) -> bool {
    let proc = Path::new("/proc");
    if proc.is_dir() {
        return match fs::read_to_string(lock) {
            Ok(pid) if !pid.trim().is_empty() => !proc.join(pid.trim()).exists(),
            _ => fs::metadata(lock)
                .and_then(|m| m.modified())
                .is_ok_and(|at| at.elapsed().is_ok_and(|age| age > Duration::from_secs(5))),
        };
    }
    fs::metadata(lock)
        .and_then(|m| m.modified())
        .is_ok_and(|at| at.elapsed().is_ok_and(|age| age > Duration::from_secs(900)))
}

fn read_cached(cache: &Path, key: &str, done: &Path) -> Result<Option<Synthesis>, String> {
    if !done.is_file() {
        return Ok(None);
    }
    let read = |suffix: &str| {
        let path = cache.join(format!("{key}.{suffix}"));
        fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
    };
    let code = read("code")?;
    Ok(Some(Synthesis {
        code: code.trim().parse().ok(),
        stdout: read("stdout")?,
        stderr: read("stderr")?,
        suite: fs::read(cache.join(format!("{key}.suite.json"))).ok(),
    }))
}

/// The record a synthesis of `<root>/ess` gives, its version read with the same `cwd` the
/// synthesis ran in. A failed synthesis is recorded too, so the gate reports its refusal.
pub fn record_of(
    root: &Path,
    ess: &Ess,
    cwd: Option<&Path>,
    synthesis: &Synthesis,
) -> Result<Record, String> {
    let version = ess_version(ess, cwd)?;
    Ok(Record {
        inputs: inputs_digest(&root.join("ess"), &version)?,
        ess: version,
        exit: synthesis
            .code
            .map_or_else(|| "signal".to_owned(), |code| code.to_string()),
        suite: synthesis
            .suite
            .as_deref()
            .map_or_else(|| NO_SUITE.to_owned(), digest),
        stdout: synthesis.stdout.clone(),
    })
}

/// Writes [`RECORD`], and [`SUITE`] when synthesize wrote one (removing a stale one when it did
/// not), under `root` from `synthesis`, a run over `<root>/ess` in `cwd`.
pub fn write_generated(
    root: &Path,
    ess: &Ess,
    cwd: Option<&Path>,
    synthesis: &Synthesis,
) -> Result<Record, String> {
    let record = record_of(root, ess, cwd, synthesis)?;
    let suite_path = root.join(SUITE);
    if let Some(dir) = suite_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    match &synthesis.suite {
        Some(suite) => fs::write(&suite_path, suite)
            .map_err(|e| format!("write {}: {e}", suite_path.display()))?,
        None if suite_path.exists() => fs::remove_file(&suite_path)
            .map_err(|e| format!("remove {}: {e}", suite_path.display()))?,
        None => {}
    }
    let record_path = root.join(RECORD);
    fs::write(&record_path, record.render())
        .map_err(|e| format!("write {}: {e}", record_path.display()))?;
    Ok(record)
}

/// A fresh synthesis of `<root>/ess` into `scratch`, run in `<root>/ess`.
pub fn fresh(root: &Path, ess: &Ess, scratch: &Path) -> Result<Synthesis, String> {
    fs::create_dir_all(scratch).map_err(|e| format!("create {}: {e}", scratch.display()))?;
    let spec = root.join("ess");
    synthesize(ess, Some(&spec), &spec, &scratch.join("suite.json"))
}

/// Synthesizes `<root>/ess` in `<root>/ess` and writes [`SUITE`] and [`RECORD`] under `root`.
/// A failed synthesis is written and then returned as the error.
pub fn generate(root: &Path, ess: &Ess, scratch: &Path) -> Result<Record, String> {
    let synthesis = fresh(root, ess, scratch)?;
    let record = write_generated(root, ess, Some(&root.join("ess")), &synthesis)?;
    if record.success() {
        Ok(record)
    } else {
        Err(format!(
            "`ess verify conform synthesize` over ess/ failed, and {RECORD} records it: {}",
            synthesis.describe()
        ))
    }
}

/// Fails unless a fresh synthesis of `<root>/ess` gives the committed [`SUITE`] and [`RECORD`]
/// byte for byte, naming each file that differs.
pub fn compare_fresh(root: &Path, ess: &Ess, scratch: &Path) -> Result<Record, String> {
    let synthesis = fresh(root, ess, scratch)?;
    let record = record_of(root, ess, Some(&root.join("ess")), &synthesis)?;
    let mut differ = Vec::new();
    if fs::read(root.join(SUITE)).ok() != synthesis.suite {
        differ.push(SUITE);
    }
    if fs::read_to_string(root.join(RECORD)).ok().as_deref() != Some(record.render().as_str()) {
        differ.push(RECORD);
    }
    if differ.is_empty() {
        Ok(record)
    } else {
        Err(format!(
            "{} differ from a fresh synthesis of ess/: regenerate with `{REGENERATE}` and commit",
            differ.join(" and ")
        ))
    }
}

/// The committed record and suite of `root` (no suite when synthesize wrote none), after checking
/// that the record's input digest is that of `<root>/ess` with the `ess` on `PATH` and its suite
/// digest that of [`SUITE`].
pub fn committed(root: &Path, ess: &Ess) -> Result<(Record, Option<String>), String> {
    let record_path = root.join(RECORD);
    let text = fs::read_to_string(&record_path).map_err(|e| {
        format!(
            "read {}: {e}; write it with `{REGENERATE}`",
            record_path.display()
        )
    })?;
    let record = Record::parse(&text).map_err(|e| {
        format!("{RECORD} is not a synthesis record ({e}); rewrite it with `{REGENERATE}`")
    })?;
    let version = ess_version(ess, None)?;
    let inputs = inputs_digest(&root.join("ess"), &version)?;
    if inputs != record.inputs {
        return Err(format!(
            "{SUITE} was synthesized from other inputs than this tree's ess/ (recorded {} with \
             `{}`, ess/ is {inputs} with `{version}`): regenerate it with `{REGENERATE}` and \
             commit both files",
            record.inputs, record.ess
        ));
    }
    if record.suite == NO_SUITE {
        return Ok((record, None));
    }
    let suite_path = root.join(SUITE);
    let suite = fs::read(&suite_path).map_err(|e| {
        format!(
            "read {}: {e}; write it with `{REGENERATE}`",
            suite_path.display()
        )
    })?;
    let found = digest(&suite);
    if found != record.suite {
        return Err(format!(
            "{SUITE} is {found}, and {RECORD} records {}: the suite was changed by hand; \
             regenerate it with `{REGENERATE}`",
            record.suite
        ));
    }
    let suite = String::from_utf8(suite).map_err(|e| format!("{SUITE} is not UTF-8: {e}"))?;
    Ok((record, Some(suite)))
}

/// Removes the decision domain from the specification at `spec`: `domains/decision.yaml`,
/// `scenarios/decision/`, their lines in `ess-inputs.yaml` and `loom.decision` in `system.yaml`.
/// Synthesizing it costs minutes (story:decision-request-entity), the rest of `ess/` about a
/// second, so the copies tests synthesize leave it out unless they are about it. A specification
/// without it is left as it is.
pub fn drop_decision(spec: &Path) -> Result<(), String> {
    let domain = spec.join("domains/decision.yaml");
    if domain.exists() {
        fs::remove_file(&domain).map_err(|e| format!("remove {}: {e}", domain.display()))?;
    }
    let scenarios = spec.join("scenarios/decision");
    if scenarios.exists() {
        fs::remove_dir_all(&scenarios)
            .map_err(|e| format!("remove {}: {e}", scenarios.display()))?;
    }
    let keep = |file: &str, drop: &dyn Fn(&str) -> bool| -> Result<(), String> {
        let path = spec.join(file);
        let text =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let kept: String = text
            .split_inclusive('\n')
            .filter(|line| !drop(line.trim_end()))
            .collect();
        fs::write(&path, kept).map_err(|e| format!("write {}: {e}", path.display()))
    };
    keep("ess-inputs.yaml", &|line| {
        line == "  - domains/decision.yaml" || line.starts_with("  - scenarios/decision/")
    })?;
    keep("system.yaml", &|line| line == "  - loom.decision")
}

/// `sha256:<hex>` of `bytes`.
pub fn digest(bytes: &[u8]) -> String {
    let mut out = String::from("sha256:");
    for byte in sha256(bytes) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// SHA-256 (FIPS 180-4).
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = bytes.to_vec();
    let bits = (bytes.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for block in message.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut out = [0u8; 32];
    for (chunk, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    out
}
