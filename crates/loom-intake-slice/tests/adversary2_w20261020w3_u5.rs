//! Adversary cases, pass 2, for `story:child-egress-ca-bundle` (wave 2026-10-20-w3, U5).
//!
//! Fix round 1 refuses a CA bundle beside an environment naming any of eight variables "that
//! replaces or widens a TLS client's trust store" (`TRUST_STORE_VARIABLES`), and documents on
//! `ChildCommand::with_environment` that beside a bundle "the environment names no variable that
//! replaces or widens a TLS client's trust store", and on the constant that "the list keeps a host
//! from undoing the bundle it names through the environment it names beside it". The list holds
//! curl's `CURL_CA_BUNDLE` and Git's `GIT_SSL_CAINFO`; these cases name other variables the same
//! two programs read for their trust anchor:
//!
//! - curl reads `CURL_HOME` for its `.curlrc`, whose `cacert` option names the CA file it trusts;
//! - Git reads `GIT_CONFIG_COUNT` with `GIT_CONFIG_KEY_<n>`/`GIT_CONFIG_VALUE_<n>`,
//!   `GIT_CONFIG_PARAMETERS` and `GIT_CONFIG_GLOBAL` for its configuration, whose
//!   `http.sslCAInfo` names the CA file it trusts, the setting `GIT_SSL_CAINFO` overrides.
//!
//! Two cases need no backend. Two are delegated, run against the real backend in a delegated
//! cgroup v2 scope the way `tests/confined_child.rs` does, and skip naming a missing prerequisite
//! (`skipped: missing prerequisite: …`). Their host is a loopback listener the test starts;
//! nothing connects past loopback and no key leaves the test's own temporary directory.

use std::io::{BufRead as _, BufReader};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use b10x_loom_intake_slice::confinement::{
    ChildCommand, ConfinementError, ConfinementRefusal, EgressAperture, ExecEnvironment,
    SubstrateRunner, TestExecution, prepare_delegated_cgroup,
};

const DELEGATED: &str = "LOOM_ADV2_W20261020W3_U5_DELEGATED";
const REPORT: &str = "LOOM_ADV2_W20261020W3_U5_REPORT";
const BUBBLEWRAP: &str = "/usr/bin/bwrap";
const OPENSSL: &str = "/usr/bin/openssl";
const CURL: &str = "/usr/bin/curl";
const GIT: &str = "/usr/bin/git";
/// The start of the refusal `run_child` gives when Substrate cannot serve the egress aperture.
const EGRESS_UNSERVED: &str = "Substrate does not serve the egress aperture";
/// The host name the aperture names; mapped to the forwarder inside the sandbox.
const NAMED_HOST: &str = "model.example.test";

fn bubblewrap_present() -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(BUBBLEWRAP).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The egress aperture `model`, naming `host` and pinned at `address`.
fn aperture_named(host: &str, address: SocketAddr) -> EgressAperture {
    EgressAperture {
        name: "model".to_owned(),
        host: host.to_owned(),
        port: address.port(),
        pinned: address,
        max_bytes: None,
    }
}

fn environment(variables: &[(&str, &str)]) -> ExecEnvironment {
    ExecEnvironment {
        allow: Vec::new(),
        set: variables
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    }
}

/// Each environment beside a valid, existing, readable bundle and a well-formed aperture is
/// refused `ScopeInvalid` before anything starts, the refusal naming `named`; the failures, one
/// line each, for every environment that is not.
fn not_refused(cases: &[(&str, ExecEnvironment)]) -> Vec<String> {
    let fixture = tempfile::tempdir().unwrap();
    let bundle = fixture.path().join("trusted.pem");
    std::fs::write(&bundle, "anchor\n").unwrap();
    // A directory that is no cgroup: a launch past the environment check is refused for it.
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let aperture = aperture_named(NAMED_HOST, SocketAddr::from((Ipv4Addr::LOCALHOST, 443)));
    let mut failures = Vec::new();
    for (case, (named, env)) in cases.iter().enumerate() {
        // A workspace of its own, so what one launch left behind is not read as the next one's.
        let workspace = fixture.path().join(format!("workspace{case}"));
        std::fs::create_dir(&workspace).unwrap();
        let child = ChildCommand::new(CURL, [] as [&str; 0], &workspace)
            .with_egress(aperture.clone())
            .with_environment(env.clone())
            .with_ca_bundle(&bundle);
        let error = runner.run_child(&child).unwrap_err();
        let target = workspace.join("target");
        if error.reason == ConfinementRefusal::ScopeInvalid && error.detail.contains(named) {
            assert!(
                !target.exists(),
                "a refused child leaves the workspace as it was"
            );
        } else {
            failures.push(format!(
                "{:?}: not refused before launch naming {named}, reached {:?} (target created: {}): {}",
                env.set,
                error.reason,
                target.exists(),
                error.detail
            ));
        }
    }
    failures
}

/// curl takes its options from `$CURL_HOME/.curlrc` before `$HOME/.curlrc`; a `cacert` line
/// there replaces the CA file curl trusts, as `CURL_CA_BUNDLE` (on the list) does. Beside a
/// bundle it must be refused before launch, naming it, as the listed variables are.
#[test]
fn adv2_a_curl_home_beside_a_ca_bundle_is_refused_as_the_listed_variables_are() {
    let failures = not_refused(&[("CURL_HOME", environment(&[("CURL_HOME", "/workspace")]))]);
    assert!(
        failures.is_empty(),
        "an environment variable through which curl takes another trust store than the named \
         bundle reaches the backend:\n{}",
        failures.join("\n")
    );
}

/// Git takes `http.sslCAInfo`, the CA file it trusts, from its configuration: from the
/// environment itself (`GIT_CONFIG_COUNT` with `GIT_CONFIG_KEY_0`/`GIT_CONFIG_VALUE_0`, and
/// `GIT_CONFIG_PARAMETERS`, the carrier of `git -c`) or from a file the environment names
/// (`GIT_CONFIG_GLOBAL`). Each replaces the trust store as `GIT_SSL_CAINFO` (on the list) does,
/// and beside a bundle must be refused before launch, naming a `GIT_CONFIG_` variable.
#[test]
fn adv2_git_configuration_beside_a_ca_bundle_is_refused_as_the_listed_variables_are() {
    let failures = not_refused(&[
        (
            "GIT_CONFIG_",
            environment(&[
                ("GIT_CONFIG_COUNT", "1"),
                ("GIT_CONFIG_KEY_0", "http.sslCAInfo"),
                ("GIT_CONFIG_VALUE_0", "/workspace/other.crt"),
            ]),
        ),
        (
            "GIT_CONFIG_PARAMETERS",
            environment(&[(
                "GIT_CONFIG_PARAMETERS",
                "'http.sslCAInfo'='/workspace/other.crt'",
            )]),
        ),
        (
            "GIT_CONFIG_GLOBAL",
            environment(&[("GIT_CONFIG_GLOBAL", "/workspace/gitconfig")]),
        ),
    ]);
    assert!(
        failures.is_empty(),
        "an environment variable through which Git takes another trust store than the named \
         bundle reaches the backend:\n{}",
        failures.join("\n")
    );
}

// --- The delegated end-to-end cases ------------------------------------------------------------

/// Run `case` against the real backend in a delegated scope, or skip naming what is missing.
fn delegated(name: &str, tools: &[&str], case: impl FnOnce(&SubstrateRunner, &Path)) {
    if std::env::var_os(DELEGATED).is_some() {
        let report = PathBuf::from(std::env::var_os(REPORT).expect("the report path is given"));
        std::fs::write(&report, "started").unwrap();
        let cgroup = match prepare_delegated_cgroup() {
            Ok(cgroup) => cgroup,
            Err(error) => {
                std::fs::write(&report, format!("a delegated cgroup v2 scope ({error})")).unwrap();
                return;
            }
        };
        let runner = SubstrateRunner::new(Some(cgroup));
        let fixture = tempfile::tempdir().unwrap();
        let workspace = fixture.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        if let Err(error) = runner.preflight(&workspace) {
            std::fs::write(&report, format!("the Substrate driver ({error})")).unwrap();
            return;
        }
        for tool in tools {
            if !Path::new(tool).is_file() {
                std::fs::write(&report, format!("executable {tool}")).unwrap();
                return;
            }
        }
        std::fs::write(&report, "ran").unwrap();
        case(&runner, &workspace);
        return;
    }
    if !bubblewrap_present() {
        eprintln!("skipped: missing prerequisite: executable {BUBBLEWRAP}");
        return;
    }
    let state = tempfile::tempdir().unwrap();
    let report = state.path().join("report");
    let started = Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(DELEGATED, "1")
        .env(REPORT, &report)
        .output();
    let output = match started {
        Ok(output) => output,
        Err(error) => {
            eprintln!("skipped: missing prerequisite: systemd-run ({error})");
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match std::fs::read_to_string(&report).ok().as_deref() {
        None => eprintln!(
            "skipped: missing prerequisite: a user systemd manager that starts a delegated scope (systemd-run {}: {})",
            output.status,
            stderr.trim()
        ),
        Some("started") => panic!(
            "the delegated case stopped before it decided its prerequisites: {}\n{stdout}{stderr}",
            output.status
        ),
        Some("ran") => {
            assert!(
                output.status.success(),
                "the delegated case failed: {}\n{stdout}{stderr}",
                output.status
            );
            assert!(
                stdout.contains("1 passed"),
                "the delegated binary ran the case: {stdout}{stderr}"
            );
            eprintln!("ran delegated: {name}");
        }
        Some(missing) => eprintln!("skipped: missing prerequisite: {missing}"),
    }
}

/// Inside the delegated binary: the case skips, naming what this machine lacks.
fn skip(missing: &str) {
    let report = std::env::var_os(REPORT).expect("a case skips only inside the delegated binary");
    std::fs::write(report, missing).unwrap();
}

/// `None` once the case skipped because Substrate does not serve the aperture here; otherwise the
/// result as it came.
fn served(
    result: Result<TestExecution, ConfinementError>,
) -> Option<Result<TestExecution, ConfinementError>> {
    match result {
        Err(error)
            if error.reason == ConfinementRefusal::CapabilityUnserved
                && error.detail.starts_with(EGRESS_UNSERVED) =>
        {
            skip(&format!("Substrate egress apertures ({error})"));
            None
        }
        result => Some(result),
    }
}

/// Make, in `directory`, two CA bundles `trusted.crt` and `other.crt`, and `host.crt`/`host.key`
/// for [`NAMED_HOST`] signed by `other`. Each is valid for a day; no key leaves the directory.
fn make_test_cas(directory: &Path) {
    let openssl = |args: &[&str]| {
        let output = Command::new(OPENSSL)
            .args(args)
            .current_dir(directory)
            .stdin(Stdio::null())
            .output()
            .expect("openssl starts");
        assert!(
            output.status.success(),
            "openssl {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let new = [
        "req",
        "-x509",
        "-nodes",
        "-days",
        "1",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:prime256v1",
    ];
    for ca in ["trusted", "other"] {
        let (key, certificate) = (format!("{ca}.key"), format!("{ca}.crt"));
        let subject = format!("/CN=loom test CA {ca}");
        let mut args = new.to_vec();
        args.extend([
            "-keyout",
            key.as_str(),
            "-out",
            certificate.as_str(),
            "-subj",
            subject.as_str(),
        ]);
        args.extend(["-addext", "basicConstraints=critical,CA:TRUE"]);
        args.extend(["-addext", "keyUsage=critical,keyCertSign"]);
        openssl(&args);
    }
    let subject = format!("/CN={NAMED_HOST}");
    let names = format!("subjectAltName=DNS:{NAMED_HOST}");
    let mut args = new.to_vec();
    args.extend([
        "-keyout",
        "host.key",
        "-out",
        "host.crt",
        "-subj",
        subject.as_str(),
    ]);
    args.extend(["-addext", names.as_str()]);
    args.extend(["-addext", "basicConstraints=critical,CA:FALSE"]);
    args.extend(["-addext", "extendedKeyUsage=serverAuth"]);
    args.extend(["-CA", "other.crt", "-CAkey", "other.key"]);
    openssl(&args);
}

/// `openssl s_server` answering HTTPS with `host.crt` on `ip`, at the port the kernel picked;
/// killed and reaped when dropped.
struct TlsHost {
    server: std::process::Child,
    address: SocketAddr,
}

impl TlsHost {
    fn start(directory: &Path, ip: Ipv4Addr) -> Self {
        let mut server = Command::new(OPENSSL)
            .args(["s_server", "-www", "-cert", "host.crt", "-key", "host.key"])
            .arg("-accept")
            .arg(format!("{ip}:0"))
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("openssl s_server starts");
        let stdout = server.stdout.take().expect("the server's output is piped");
        let (port, reported) = std::sync::mpsc::channel();
        let accept = format!("ACCEPT {ip}:");
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(number) = line.strip_prefix(&accept).and_then(|p| p.parse().ok()) {
                    let _ = port.send(number);
                }
            }
        });
        match reported.recv_timeout(Duration::from_secs(30)) {
            Ok(port) => Self {
                server,
                address: SocketAddr::from((ip, port)),
            },
            Err(error) => {
                let _ = server.kill();
                let _ = server.wait();
                panic!("openssl s_server reported no port it accepts on: {error}");
            }
        }
    }
}

impl Drop for TlsHost {
    fn drop(&mut self) {
        let _ = self.server.kill();
        let _ = self.server.wait();
    }
}

/// The host, signed by `other`, and the `trusted` bundle the host names: a child trusting only
/// the named bundle fails to verify it. `other.crt` is copied into the workspace, where an
/// environment variable can point a program at it.
fn host_signed_by_another_ca(workspace: &Path) -> (tempfile::TempDir, TlsHost, PathBuf) {
    let anchors = tempfile::tempdir().unwrap();
    make_test_cas(anchors.path());
    std::fs::copy(
        anchors.path().join("other.crt"),
        workspace.join("other.crt"),
    )
    .unwrap();
    let host = TlsHost::start(anchors.path(), Ipv4Addr::new(127, 0, 0, 2));
    let trusted = anchors.path().join("trusted.crt");
    (anchors, host, trusted)
}

/// The host names the `trusted` bundle; the loopback TLS host serves a certificate the *other*
/// CA signed. The control, curl with the host's environment empty, fails verification (exit 60).
/// Then the workspace holds a `.curlrc` saying `cacert = /workspace/other.crt` and the child's
/// environment names `CURL_HOME=/workspace`, one variable not on the list. The story and the
/// `with_environment` doc hold only if that launch is refused before it starts or curl still
/// fails verification; it is red when curl verifies the host against the CA the host never named.
#[test]
fn adv2_a_curl_home_beside_the_named_bundle_makes_curl_trust_a_ca_the_host_never_named() {
    delegated(
        "adv2_a_curl_home_beside_the_named_bundle_makes_curl_trust_a_ca_the_host_never_named",
        &[OPENSSL, CURL],
        |runner, workspace| {
            let (_anchors, host, trusted) = host_signed_by_another_ca(workspace);
            std::fs::write(workspace.join(".curlrc"), "cacert = /workspace/other.crt\n").unwrap();
            let url = format!("https://{NAMED_HOST}:{}/", host.address.port());
            let args = [
                "--silent",
                "--show-error",
                "--max-time",
                "20",
                "--output",
                "/dev/null",
                "--write-out",
                "status:%{http_code}\n",
                url.as_str(),
            ];
            let child = ChildCommand::new(CURL, args, workspace)
                .with_egress(aperture_named(NAMED_HOST, host.address))
                .with_ca_bundle(&trusted);

            let Some(control) = served(runner.run_child(&child)) else {
                return;
            };
            let control = control.expect("the control runs confined with its egress");
            assert_eq!(
                control.exit_code,
                Some(60),
                "the control fails verification: {}",
                control.output
            );

            let redirected = child.with_environment(environment(&[("CURL_HOME", "/workspace")]));
            match runner.run_child(&redirected) {
                Err(error) => {
                    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
                    assert!(error.detail.contains("CURL_HOME"), "{error}");
                }
                Ok(ran) => assert_eq!(
                    ran.exit_code,
                    Some(60),
                    "CURL_HOME beside the named bundle made curl verify the host against a CA \
                     the host never named: {}",
                    ran.output
                ),
            }
        },
    );
}

/// The host names the `trusted` bundle; the loopback TLS host serves a certificate the *other*
/// CA signed. The control, `git ls-remote` with the host's environment empty, fails certificate
/// verification. Then the child's environment names `GIT_CONFIG_COUNT=1`,
/// `GIT_CONFIG_KEY_0=http.sslCAInfo` and `GIT_CONFIG_VALUE_0=/workspace/other.crt`, no file
/// needed. The story and the `with_environment` doc hold only if that launch is refused before it
/// starts or Git still fails verification; it is red when Git gets past TLS (the host's `-www`
/// page is no repository, so Git then says `not valid: is this a git repository?`).
#[test]
fn adv2_git_configuration_beside_the_named_bundle_makes_git_trust_a_ca_the_host_never_named() {
    delegated(
        "adv2_git_configuration_beside_the_named_bundle_makes_git_trust_a_ca_the_host_never_named",
        &[OPENSSL, GIT],
        |runner, workspace| {
            let (_anchors, host, trusted) = host_signed_by_another_ca(workspace);
            let url = format!("https://{NAMED_HOST}:{}/repo", host.address.port());
            let child = ChildCommand::new(GIT, ["ls-remote", url.as_str()], workspace)
                .with_egress(aperture_named(NAMED_HOST, host.address))
                .with_ca_bundle(&trusted)
                .with_timeout(Duration::from_secs(60));

            let Some(control) = served(runner.run_child(&child)) else {
                return;
            };
            let control = control.expect("the control runs confined with its egress");
            assert!(
                control.output.contains("SSL certificate"),
                "the control fails certificate verification: {:?} {}",
                control.exit_code,
                control.output
            );

            let redirected = child.with_environment(environment(&[
                ("GIT_CONFIG_COUNT", "1"),
                ("GIT_CONFIG_KEY_0", "http.sslCAInfo"),
                ("GIT_CONFIG_VALUE_0", "/workspace/other.crt"),
            ]));
            match runner.run_child(&redirected) {
                Err(error) => {
                    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
                    assert!(error.detail.contains("GIT_CONFIG_"), "{error}");
                }
                Ok(ran) => assert!(
                    ran.output.contains("SSL certificate") && !ran.output.contains("not valid"),
                    "GIT_CONFIG_* beside the named bundle made Git verify the host against a CA \
                     the host never named: {:?} {}",
                    ran.exit_code,
                    ran.output
                ),
            }
        },
    );
}
