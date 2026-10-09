//! Reads through the `connectors` command line (`story:connectors-cli-reads`).
//!
//! [`ConnectorsCli`] reads a [`DataSource`] the host configured, outside a run (a poll) or inside
//! one (a turn's `source.read`). It needs no `AdmittedRequest` and no binding: it is a plain read
//! client over the operator's own `connectors`, whose keyring holds the credential. It reads no
//! credential, passes none, and hands the program only the environment variables it names in
//! [`INHERITED`].
//!
//! One read is two commands, each `connectors --output json [--config F] [--state-dir D]` and then:
//!
//! 1. `operations describe --adapter A --operation O`, which answers the operation's schema
//!    identity (`schema`), the descriptor `revision` and the operation with its `profile` and
//!    `input_schema` (connectors `v0.38.0`, `apps/connectors/src/local/operations.rs`, `describe`
//!    and `operation`);
//! 2. `operations invoke --adapter A --connection C --operation O --schema S --revision R
//!    --input-stdin`, run once, with the input document on stdin, never in argv.
//!
//! An operation `describe` reports with the `mutation` profile ([`WRITE_PROFILES`]) is refused
//! after the describe and before any invoke; Connectors itself refuses any write without
//! `--approval-file`, which this client never passes, and that holds a write an adapter publishes
//! under another profile. A program that forks is bounded only as the process it started: a
//! timeout kills and reaps that process, not its children. The command line's argv
//! holds only the flags above and the ids they take; an id, or a schema or revision `describe`
//! answered, that is empty, starts with `-`, or holds whitespace or a control character starts no
//! command, and a `--config` or `--state-dir` path starting with `-` starts none either. Each
//! command is bounded by `timeout_seconds` ([`DEFAULT_TIMEOUT_SECONDS`] when absent): one still
//! running is killed and reaped, and the read fails naming the timeout and the operation. Nothing
//! is retried: a refusal Connectors states comes back as a [`ReadRefusal`] naming the source, and
//! `not_granted`'s names its remedy, `connectors connections revalidate --adapter <alias>`, which
//! the client never runs itself.

use std::ffi::OsString;
use std::fmt;
use std::io::{Read, Write};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub use loom::datasource;
pub use loom::json;

use loom::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, DataSource, OperationId, ReadKind,
    ReadRefusal, ReadResult, SourceEntity, SourceName,
};
use loom::json::Value;

/// The only environment variables the program is started with, taken from the environment the
/// client was given: where the operator's configuration, state, keyring session and programs are.
/// None of them is a credential, and every other variable (a token included) is dropped.
pub const INHERITED: &[&str] = &[
    "HOME",
    "PATH",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TZ",
    "XDG_CONFIG_HOME",
    "XDG_STATE_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// The `profile`s an operation is refused under, as `operations describe` prints them in
/// `result.operation.profile`: `mutation`, the catalog adapter's profile for a write (connectors
/// `v0.38.0`, `adapters/catalog/src/lib.rs:1233`, `Effect::Write => "mutation"`; reads are
/// `generic-http`), and the profile `contracts/service/compatibility.md` § 1 names.
///
/// `operations describe` does not print the effect itself, and adapters publish other profiles
/// (`resource` among them) for reads and writes alike, so those are read. What holds a write an
/// adapter publishes under another profile is Connectors' own check: it refuses any write without
/// `--approval-file` (`v0.38.0`, `crates/connectors-host/src/local/owner/mutation/execution.rs:161`,
/// `proof.ok_or(ApprovalCode::ApprovalRequired)`), and this client never builds that flag.
pub const WRITE_PROFILES: &[&str] = &["mutation"];

/// The bound on one command when the configuration names none.
pub const DEFAULT_TIMEOUT_SECONDS: i64 = 60;

/// What [`ConnectorsCli::sources`] lists for one source: each kind it declares, in the order list,
/// search, get, with what `operations describe` reports for that kind's operation.
pub type SourceEntities = Vec<(ReadKind, SourceEntity)>;

/// Why a read answered nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// The read was not made, or Connectors refused it with a failure code.
    Refused(ReadRefusal),
    /// The read could not be made or understood: the program did not start, answered no JSON or
    /// an answer without the member it needs, or an argument would have read as a flag.
    Failed(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => {
                write!(f, "refused ({}): {}", refusal.code, refusal.message)
            }
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CliError {}

/// A read client over the `connectors` command line and the host's configured sources.
#[derive(Debug, Clone)]
pub struct ConnectorsCli {
    config: ConnectorsCliConfig,
    sources: Vec<DataSource>,
    environment: Vec<(OsString, OsString)>,
}

impl ConnectorsCli {
    /// A client running `config.program` over `sources`, with this process's environment
    /// narrowed to [`INHERITED`].
    pub fn new(config: ConnectorsCliConfig, sources: Vec<DataSource>) -> Self {
        Self {
            config,
            sources,
            environment: Vec::new(),
        }
        .with_environment(std::env::vars_os())
    }

    /// Starts the program from `environment` instead of this process's, still narrowed to
    /// [`INHERITED`].
    #[must_use]
    pub fn with_environment<K, V>(mut self, environment: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.environment = environment
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .filter(|(name, _)| INHERITED.iter().any(|kept| name == kept))
            .collect();
        self
    }

    /// The configured sources, each with the [`SourceEntity`] `operations describe` reports for
    /// every kind it declares, in the order list, search, get.
    ///
    /// # Errors
    /// The first describe that fails or is refused, a write operation included.
    pub fn sources(&self) -> Result<Vec<(DataSource, SourceEntities)>, CliError> {
        self.sources
            .iter()
            .map(|source| {
                let entities = [ReadKind::List, ReadKind::Search, ReadKind::Get]
                    .into_iter()
                    .filter_map(|kind| declared(source, kind).map(|operation| (kind, operation)))
                    .map(|(kind, operation)| {
                        self.describe_for(Some(&source.name), &source.adapter, operation)
                            .map(|entity| (kind, entity))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((source.clone(), entities))
            })
            .collect()
    }

    /// What `operations describe` reports for the read operation `operation` of `adapter`.
    ///
    /// # Errors
    /// [`CliError::Refused`] with code `write-operation` for an operation described with the
    /// `mutation` profile, or Connectors' own code; [`CliError::Failed`] otherwise.
    pub fn describe(
        &self,
        adapter: &AdapterAlias,
        operation: &OperationId,
    ) -> Result<SourceEntity, CliError> {
        self.describe_for(None, adapter, operation)
    }

    /// Reads `operation` of `adapter` on `connection`: one describe, then exactly one
    /// `operations invoke` with `input` on stdin.
    ///
    /// # Errors
    /// As [`ConnectorsCli::describe`], and the invoke's refusal or failure.
    pub fn invoke_read(
        &self,
        adapter: &AdapterAlias,
        connection: &ConnectionId,
        operation: &OperationId,
        input: &Value,
    ) -> Result<ReadResult, CliError> {
        self.read_through(None, adapter, connection, operation, input)
    }

    /// Reads `source` through the operation it declares for `kind`.
    ///
    /// # Errors
    /// [`CliError::Refused`] with code `undeclared-kind`, before any command, when `source`
    /// declares no operation for `kind`; otherwise as [`ConnectorsCli::invoke_read`], every
    /// refusal naming the source.
    pub fn read(
        &self,
        source: &DataSource,
        kind: ReadKind,
        input: &Value,
    ) -> Result<ReadResult, CliError> {
        let Some(operation) = declared(source, kind) else {
            return Err(CliError::Refused(ReadRefusal {
                source: Some(source.name.clone()),
                adapter: source.adapter.clone(),
                operation: None,
                code: "undeclared-kind".to_owned(),
                message: format!(
                    "source `{}` declares no {} operation",
                    source.name.0,
                    kind_name(kind)
                ),
            }));
        };
        self.read_through(
            Some(&source.name),
            &source.adapter,
            &source.connection,
            operation,
            input,
        )
    }

    fn describe_for(
        &self,
        source: Option<&SourceName>,
        adapter: &AdapterAlias,
        operation: &OperationId,
    ) -> Result<SourceEntity, CliError> {
        argument("adapter", &adapter.0)?;
        argument("operation", &operation.0)?;
        let described = self.run(
            source,
            adapter,
            operation,
            &[
                "operations",
                "describe",
                "--adapter",
                &adapter.0,
                "--operation",
                &operation.0,
            ],
            None,
        )?;
        let selected = described
            .member("operation")
            .ok_or_else(|| missing("describe", "operation"))?;
        if text(selected, "id") != Some(operation.0.as_str()) {
            return Err(CliError::Failed(format!(
                "describe of `{}` answered another operation",
                operation.0
            )));
        }
        if let Some(answered) = described.member("adapter")
            && answered != &Value::Text(adapter.0.clone())
        {
            return Err(CliError::Failed(format!(
                "describe of adapter `{}` answered another adapter",
                adapter.0
            )));
        }
        // `result.operation.profile` decides a write (connectors `v0.38.0`,
        // `apps/connectors/src/local/operations.rs` `summary`; see [`WRITE_PROFILES`]); an
        // operation without one is not read either.
        let profile = text(selected, "profile").ok_or_else(|| missing("describe", "profile"))?;
        if WRITE_PROFILES.contains(&profile) {
            return Err(CliError::Refused(ReadRefusal {
                source: source.cloned(),
                adapter: adapter.clone(),
                operation: Some(operation.clone()),
                code: "write-operation".to_owned(),
                message: format!(
                    "{} is described with the `{profile}` profile, a write, and is never invoked",
                    subject(source, adapter, Some(operation))
                ),
            }));
        }
        let input_schema = match selected.member("input_schema") {
            // Printed as the JSON Schema's text (`operation`: `input_schema.to_string()`).
            Some(Value::Text(schema)) => json::parse(schema).map_err(|error| {
                CliError::Failed(format!(
                    "describe gave an input schema that is no JSON: {error}"
                ))
            })?,
            Some(schema @ Value::Object(_)) => schema.clone(),
            _ => return Err(missing("describe", "input_schema")),
        };
        Ok(SourceEntity {
            operation: operation.clone(),
            schema: text(&described, "schema")
                .ok_or_else(|| missing("describe", "schema"))?
                .to_owned(),
            input_schema,
            revision: text(&described, "revision")
                .ok_or_else(|| missing("describe", "revision"))?
                .to_owned(),
        })
    }

    fn read_through(
        &self,
        source: Option<&SourceName>,
        adapter: &AdapterAlias,
        connection: &ConnectionId,
        operation: &OperationId,
        input: &Value,
    ) -> Result<ReadResult, CliError> {
        argument("adapter", &adapter.0)?;
        argument("connection", &connection.0)?;
        argument("operation", &operation.0)?;
        let entity = self.describe_for(source, adapter, operation)?;
        argument("schema", &entity.schema)?;
        argument("revision", &entity.revision)?;
        let mut document = String::new();
        json::push_value(&mut document, input);
        let invoked = self.run(
            source,
            adapter,
            operation,
            &[
                "operations",
                "invoke",
                "--adapter",
                &adapter.0,
                "--connection",
                &connection.0,
                "--operation",
                &operation.0,
                "--schema",
                &entity.schema,
                "--revision",
                &entity.revision,
                "--input-stdin",
            ],
            Some(&document),
        )?;
        if let Some(answered) = invoked.member("operation")
            && answered != &Value::Text(operation.0.clone())
        {
            return Err(CliError::Failed(format!(
                "invoke of `{}` answered another operation",
                operation.0
            )));
        }
        // A read's answer is the provider's JSON value itself, never its text (connectors
        // `v0.38.0`, `crates/connectors-host/src/local/owner/supervisor.rs` `read_answer`).
        let body = invoked
            .member("result")
            .ok_or_else(|| missing("invoke", "result"))?
            .clone();
        // `source_audit.audit_ref`: the member the command line prints the execution audit record
        // in (connectors `v0.38.0`, `apps/connectors/src/local/operations.rs` `project`;
        // `docs/local-gitlab-merge.md`: success "adds `request_id`, `mutation` and
        // `source_audit`"). `operations invoke --help` names no audit member, and at `v0.38.0` a
        // read prints none, so it is absent there.
        let audit_ref = invoked
            .member("source_audit")
            .and_then(|audit| text(audit, "audit_ref"))
            .map(str::to_owned);
        Ok(ReadResult { body, audit_ref })
    }

    /// Runs one command and answers the `result` of its JSON answer.
    fn run(
        &self,
        source: Option<&SourceName>,
        adapter: &AdapterAlias,
        operation: &OperationId,
        arguments: &[&str],
        stdin: Option<&str>,
    ) -> Result<Value, CliError> {
        let mut command = Command::new(&self.config.program);
        command
            .env_clear()
            .envs(self.environment.iter().map(|(name, value)| (name, value)))
            .args(["--output", "json"]);
        for (flag, path) in [
            ("--config", &self.config.config),
            ("--state-dir", &self.config.state_dir),
        ] {
            if let Some(path) = path {
                // A path is one argv element: only a leading `-` could read as a flag.
                if path.starts_with('-') {
                    return Err(CliError::Failed(format!(
                        "{flag} `{path}` cannot be passed to connectors"
                    )));
                }
                command.arg(flag).arg(path);
            }
        }
        let seconds = self
            .config
            .timeout_seconds
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS);
        let timeout = u64::try_from(seconds)
            .ok()
            .filter(|seconds| *seconds > 0)
            .map(Duration::from_secs)
            .ok_or_else(|| {
                CliError::Failed(format!("timeout_seconds {seconds} is not a positive bound"))
            })?;
        command
            .args(arguments)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output =
            Self::bounded(&mut command, stdin, timeout).map_err(|failure| match failure {
                Bounded::TimedOut => CliError::Failed(format!(
                    "`{}` timed out after {seconds} s and was stopped, for {}",
                    self.config.program,
                    subject(source, adapter, Some(operation))
                )),
                Bounded::Io(what, error) => {
                    CliError::Failed(format!("`{}` {what}: {error}", self.config.program))
                }
            })?;
        // A refusal is printed on stderr, with stdout empty.
        let printed = if output.stdout.trim_ascii().is_empty() {
            &output.stderr
        } else {
            &output.stdout
        };
        let printed = String::from_utf8_lossy(printed);
        let answer = json::parse(printed.trim()).map_err(|_| {
            CliError::Failed(format!(
                "connectors answered no JSON (exit {}) for {}",
                output.status,
                subject(source, adapter, Some(operation))
            ))
        })?;
        if output.status.success() {
            if answer.member("ok") != Some(&Value::Bool(true)) {
                return Err(missing("connectors", "ok"));
            }
            return answer
                .member("result")
                .cloned()
                .ok_or_else(|| missing("connectors", "result"));
        }
        // `{"ok": false, "error": {"code": "failure", "data": {"code": "not_granted", …}}}`.
        let error = answer.member("error");
        let code = error
            .and_then(|error| error.member("data"))
            .and_then(|data| text(data, "code"))
            .or_else(|| error.and_then(|error| text(error, "code")))
            .ok_or_else(|| {
                CliError::Failed(format!(
                    "connectors failed (exit {}) without a failure code for {}",
                    output.status,
                    subject(source, adapter, Some(operation))
                ))
            })?;
        let mut message = format!(
            "connectors refused {} with `{code}`",
            subject(source, adapter, Some(operation))
        );
        if code == "not_granted" {
            // The connection's validation evidence lapsed. Renewing it is the operator's call,
            // never this client's: it does not revalidate and does not retry.
            message.push_str(&format!(
                "; renew the connection with `connectors connections revalidate --adapter {}`",
                adapter.0
            ));
        }
        Err(CliError::Refused(ReadRefusal {
            source: source.cloned(),
            adapter: adapter.clone(),
            operation: Some(operation.clone()),
            code: code.to_owned(),
            message,
        }))
    }

    /// Runs `command` to its end or for `timeout`, whichever is first, writing `stdin` to it.
    /// A program still running at the bound is killed and reaped. The pipes are read on their own
    /// threads, so neither a program that answers before it reads nor one that holds a pipe open
    /// past its exit blocks the caller beyond the bound.
    fn bounded(
        command: &mut Command,
        stdin: Option<&str>,
        timeout: Duration,
    ) -> Result<Output, Bounded> {
        let deadline = Instant::now() + timeout;
        let mut child = command
            .spawn()
            .map_err(|error| Bounded::Io("did not start", error))?;
        if let (Some(document), Some(mut pipe)) = (stdin, child.stdin.take()) {
            let document = document.to_owned();
            // Dropping the pipe at the end of the write closes stdin; a write the program never
            // reads ends with the program.
            thread::spawn(move || pipe.write_all(document.as_bytes()));
        }
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
                Ok(None) => {
                    stop(&mut child);
                    return Err(Bounded::TimedOut);
                }
                Err(error) => {
                    stop(&mut child);
                    return Err(Bounded::Io("could not be waited for", error));
                }
            }
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        let stdout = stdout
            .recv_timeout(remaining)
            .map_err(|_| Bounded::TimedOut)?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        let stderr = stderr
            .recv_timeout(remaining)
            .map_err(|_| Bounded::TimedOut)?;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }
}

/// How often a running command is checked against its bound.
const POLL: Duration = Duration::from_millis(10);

/// Why a bounded command gave no output.
enum Bounded {
    /// Still running, or its pipes still open, at the bound.
    TimedOut,
    /// What failed, and the error.
    Io(&'static str, std::io::Error),
}

/// Reads `pipe` to its end on its own thread; the bytes arrive on the receiver.
fn drain(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        let _ = sender.send(bytes);
    });
    receiver
}

/// Kills `child` and reaps it, so no process and no zombie is left.
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// The operation `source` declares for `kind`.
fn declared(source: &DataSource, kind: ReadKind) -> Option<&OperationId> {
    match kind {
        ReadKind::List => source.list.as_ref(),
        ReadKind::Search => source.search.as_ref(),
        ReadKind::Get => source.get.as_ref(),
    }
}

fn kind_name(kind: ReadKind) -> &'static str {
    match kind {
        ReadKind::List => "list",
        ReadKind::Search => "search",
        ReadKind::Get => "get",
    }
}

/// What a message names: the source when the read named one, else the adapter.
fn subject(
    source: Option<&SourceName>,
    adapter: &AdapterAlias,
    operation: Option<&OperationId>,
) -> String {
    let operation = operation.map_or_else(String::new, |operation| {
        format!(" operation `{}`", operation.0)
    });
    match source {
        Some(source) => format!("source `{}`{operation}", source.0),
        None => format!("adapter `{}`{operation}", adapter.0),
    }
}

/// A value that may stand in argv: non-empty, not starting with `-`, and without whitespace or a
/// control character, so it can never read as another flag.
fn argument(name: &str, value: &str) -> Result<(), CliError> {
    if value.is_empty()
        || value.starts_with('-')
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(CliError::Failed(format!(
            "{name} `{value}` cannot be passed to connectors"
        )));
    }
    Ok(())
}

fn text<'v>(value: &'v Value, name: &str) -> Option<&'v str> {
    match value.member(name) {
        Some(Value::Text(text)) => Some(text),
        _ => None,
    }
}

fn missing(answer: &str, member: &str) -> CliError {
    CliError::Failed(format!("{answer} gave no `{member}`"))
}
