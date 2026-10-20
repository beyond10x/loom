//! Adversary cases for `story:child-egress-ca-bundle` (wave 2026-10-20-w3, U5).
//!
//! The story's promise is that a child under an egress aperture "trusts exactly the named bundle"
//! and "must not fall back to the host's system store" (its Acceptance, and the `with_ca_bundle`
//! doc). `check_ca_bundle` polices the child's environment for one variable that would subvert the
//! trust anchor, `SSL_CERT_FILE`, because Substrate sets it and the host's value "would replace" it.
//! These cases probe whether that policing is complete, and whether the documented refusal order
//! holds.
//!
//! One case is delegated, run against the real backend in a delegated cgroup v2 scope the way
//! `tests/confined_child.rs` does: the test binary starts itself again under
//! `systemd-run --user --scope -p Delegate=yes` and runs the case there. Where bubblewrap, a user
//! systemd manager, the Substrate driver, Substrate's egress aperture, `openssl` or `curl` is
//! absent it prints `skipped: missing prerequisite: …`. Its host is a loopback listener the test
//! starts; nothing connects past loopback and no key leaves the test's own temporary directory.

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use b10x_loom_intake_slice::confinement::{
    ChildCommand, ConfinementError, ConfinementRefusal, EgressAperture, ExecEnvironment,
    SubstrateRunner, TestExecution, prepare_delegated_cgroup,
};

const DELEGATED: &str = "LOOM_ADV_W20261020W3_U5_DELEGATED";
const REPORT: &str = "LOOM_ADV_W20261020W3_U5_REPORT";
const BUBBLEWRAP: &str = "/usr/bin/bwrap";
const OPENSSL: &str = "/usr/bin/openssl";
const CURL: &str = "/usr/bin/curl";
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

/// An aperture whose shape passes every pre-launch check: a host name pinned to a loopback IPv4.
fn well_formed_aperture() -> EgressAperture {
    aperture_named(NAMED_HOST, SocketAddr::from((Ipv4Addr::LOCALHOST, 443)))
}

/// A child that must verify a host, named with a valid, existing, readable CA bundle and aperture:
/// everything `check_ca_bundle` asks for, so only the environment under test can refuse it.
fn well_anchored_child(workspace: &Path, bundle: &Path, env: ExecEnvironment) -> ChildCommand {
    ChildCommand::new("curl", [] as [&str; 0], workspace)
        .with_egress(well_formed_aperture())
        .with_environment(env)
        .with_ca_bundle(bundle)
}

fn env_naming(name: &str, value: &str) -> ExecEnvironment {
    ExecEnvironment {
        allow: Vec::new(),
        set: BTreeMap::from([(name.to_owned(), value.to_owned())]),
    }
}

/// A child "trusts exactly the named bundle" only if every environment variable that redirects or
/// augments a TLS client's trust anchors is refused before launch, as `SSL_CERT_FILE` is. The
/// implementation refuses only `SSL_CERT_FILE`; `CURL_CA_BUNDLE` and `CURL_CA_PATH` (curl, the
/// tool the acceptance test itself uses), `SSL_CERT_DIR` (OpenSSL), `REQUESTS_CA_BUNDLE` (Python)
/// and `NODE_EXTRA_CA_CERTS` (Node) all point a client at certificates the host never named. This
/// case asserts each such environment is refused `ScopeInvalid` before anything starts, the way the
/// `SSL_CERT_FILE` control is. It is run with a non-cgroup root, so a child that passes
/// `check_ca_bundle` fails later at the backend (`CgroupUndelegated`/`BackendMissing`): a case that
/// reaches that error is a case the environment check let through.
#[test]
fn adv_an_environment_redirecting_the_trust_anchor_is_not_refused_as_ssl_cert_file_is() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let bundle = fixture.path().join("trusted.pem");
    std::fs::write(&bundle, "anchor\n").unwrap();
    // A directory that is no cgroup: a launch that got past the environment check is refused for it.
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));

    // Control: Substrate sets SSL_CERT_FILE, so the host's value would be replaced, and the
    // implementation refuses it before launch. This passes against the code as written.
    let control = well_anchored_child(
        &workspace,
        &bundle,
        env_naming("SSL_CERT_FILE", "/workspace/other.pem"),
    );
    let error = runner.run_child(&control).unwrap_err();
    assert_eq!(
        error.reason,
        ConfinementRefusal::ScopeInvalid,
        "the SSL_CERT_FILE control is refused before launch: {error}"
    );

    // Every other variable that redirects a TLS client's trust anchors, by the same reasoning.
    for variable in [
        "CURL_CA_BUNDLE",
        "CURL_CA_PATH",
        "SSL_CERT_DIR",
        "REQUESTS_CA_BUNDLE",
        "NODE_EXTRA_CA_CERTS",
    ] {
        let child = well_anchored_child(
            &workspace,
            &bundle,
            env_naming(variable, "/workspace/other.pem"),
        );
        let error = runner.run_child(&child).unwrap_err();
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "an environment naming {variable} redirects the child's trust anchor away from the \
             named bundle and must be refused before launch, as SSL_CERT_FILE is; instead the \
             child reached the backend: {error}"
        );
        assert!(
            error.detail.contains(variable),
            "the refusal names {variable}: {error}"
        );
        assert!(
            !workspace.join("target").exists(),
            "a refused child leaves the workspace as it was"
        );
    }
}

/// The `check_ca_bundle` doc states its refusals happen "in this order: 1. a bundle for a child
/// with no egress aperture …; 2. a path that is not UTF-8, or not absolute …". A child with no
/// aperture and a non-UTF-8 bundle path therefore must be refused for the missing aperture (step
/// 1). The code checks UTF-8 first, so it is refused for the encoding (step 2) instead: the two
/// never co-occur in the unit's own tests, so nothing catches the drift.
#[test]
fn adv_a_bundle_without_an_aperture_is_refused_for_the_aperture_before_its_encoding() {
    use std::os::unix::ffi::OsStrExt as _;
    let non_utf8 = PathBuf::from(std::ffi::OsStr::from_bytes(b"/anchors/bundle\xff.pem"));
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    // No `with_egress`: the child names a bundle but no aperture.
    let child = ChildCommand::new("curl", [] as [&str; 0], &workspace).with_ca_bundle(&non_utf8);
    let error = SubstrateRunner::new(Some(fixture.path().to_owned()))
        .run_child(&child)
        .unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(
        error.detail.contains("no egress aperture"),
        "the documented order refuses the missing aperture (step 1) before the encoding (step 2): \
         {error}"
    );
}

// --- The delegated end-to-end case -------------------------------------------------------------

/// Run `case` against the real backend in a delegated scope, or skip naming what is missing.
fn delegated(name: &str, case: impl FnOnce(&SubstrateRunner, &Path)) {
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
        for tool in [OPENSSL, CURL] {
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

/// `None` once the case skipped because Substrate does not serve the aperture here.
fn egress_result(result: Result<TestExecution, ConfinementError>) -> Option<TestExecution> {
    match result {
        Err(error)
            if error.reason == ConfinementRefusal::CapabilityUnserved
                && error.detail.starts_with(EGRESS_UNSERVED) =>
        {
            skip(&format!("Substrate egress apertures ({error})"));
            None
        }
        result => Some(result.expect("the child runs confined with its egress")),
    }
}

/// Make, in `directory`, two CA bundles `trusted.crt` and `other.crt`, and `host.crt`/`host.key`
/// for [`NAMED_HOST`] signed by `signer` (`"trusted"` or `"other"`). Each is valid for a day; no
/// key leaves the directory.
fn make_test_cas(directory: &Path, signer: &str) {
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
        "req", "-x509", "-nodes", "-days", "1", "-newkey", "ec", "-pkeyopt",
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
    let signer_crt = format!("{signer}.crt");
    let signer_key = format!("{signer}.key");
    let mut args = new.to_vec();
    args.extend(["-keyout", "host.key", "-out", "host.crt", "-subj", subject.as_str()]);
    args.extend(["-addext", names.as_str()]);
    args.extend(["-addext", "basicConstraints=critical,CA:FALSE"]);
    args.extend(["-addext", "extendedKeyUsage=serverAuth"]);
    args.extend(["-CA", signer_crt.as_str(), "-CAkey", signer_key.as_str()]);
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

/// A `curl` child fetching `https://model.example.test:<port>/`, printing the HTTP status.
fn curl_child(workspace: &Path, port: u16, env: ExecEnvironment) -> ChildCommand {
    let url = format!("https://{NAMED_HOST}:{port}/");
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
    ChildCommand::new(CURL, args, workspace).with_environment(env)
}

/// The host names the `trusted` bundle, so the child must trust only it. The loopback TLS host
/// serves a certificate signed by the *other* CA, which is not in `trusted`, so verification must
/// fail. The child's environment carries `CURL_CA_BUNDLE=/workspace/other.crt`, pointing curl at
/// the other CA, which the host placed in the workspace — the one environment the unit does not
/// refuse. If curl honours it, the child verifies a certificate the host never named a trust
/// anchor for, which the story forbids ("trusts exactly the named bundle"). The case asserts the
/// story: curl fails verification (exit 60). It is red when `CURL_CA_BUNDLE` overrides the anchor.
#[test]
fn adv_a_curl_ca_bundle_in_the_environment_overrides_the_named_trust_anchor() {
    delegated(
        "adv_a_curl_ca_bundle_in_the_environment_overrides_the_named_trust_anchor",
        |runner, workspace| {
            let anchors = tempfile::tempdir().unwrap();
            // The host's certificate is signed by `other`, the CA the host does NOT name.
            make_test_cas(anchors.path(), "other");
            let trusted = anchors.path().join("trusted.crt");
            // The host places the other CA inside the workspace, readable by the child.
            std::fs::copy(anchors.path().join("other.crt"), workspace.join("other.crt")).unwrap();
            let host = TlsHost::start(anchors.path(), Ipv4Addr::new(127, 0, 0, 2));
            let aperture = aperture_named(NAMED_HOST, host.address);

            let child = curl_child(
                workspace,
                host.address.port(),
                env_naming("CURL_CA_BUNDLE", "/workspace/other.crt"),
            )
            .with_egress(aperture)
            .with_ca_bundle(&trusted);

            let Some(result) = egress_result(runner.run_child(&child)) else {
                return;
            };
            assert_eq!(
                result.exit_code,
                Some(60),
                "the child must trust only the named `trusted` bundle and reject the \
                 `other`-signed host; a CURL_CA_BUNDLE in the environment must not redirect its \
                 trust anchor: {}",
                result.output
            );
        },
    );
}
