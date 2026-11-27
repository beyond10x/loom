//! Local, read-only observation of the background agents of one user.
//!
//! Every long-lived host participates in listener election. Its state lease serializes writers
//! of that agent's record, while the observation directory scopes discovery to one OS user.
mod http;
mod snapshot;
mod store;

use b10x_loom_plugin::observe::TurnObserver;
use chrono::{SecondsFormat, Utc};
use loom_sdk::agent::AgentSpec;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub(super) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
pub(super) fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn random() -> io::Result<String> {
    let mut bytes = [0; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(digest(&bytes))
}

fn invalid(reason: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

fn root() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("LOOM_DASHBOARD_DIR") {
        return Ok(dir.into());
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|dir| PathBuf::from(dir).join(".local/state")))
        .ok_or_else(|| invalid("set LOOM_DASHBOARD_DIR when HOME and XDG_STATE_HOME are absent"))?;
    Ok(base.join("b10x-loom/dashboard"))
}

fn private_dir(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    let meta = path.symlink_metadata()?;
    // SAFETY: geteuid reads the process's effective uid and takes no arguments.
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err(invalid(
            "dashboard directory must be owned by this user and mode 0700",
        ));
    }
    Ok(())
}

fn private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

pub(super) fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().ok_or_else(|| invalid("missing parent"))?)?;
    file.write_all(contents)?;
    file.persist(path)?;
    Ok(())
}

pub(super) fn save(path: &Path, data: &impl Serialize) -> io::Result<()> {
    atomic_write(path, &serde_json::to_vec(data)?)
}

#[derive(Clone)]
struct Scope {
    root: PathBuf,
    addr: SocketAddr,
    token_hash: String,
    id: String,
}

impl Scope {
    /// Called under the scope initialization lock before the first process advertises its URL.
    fn open(
        root: PathBuf,
        desired: Option<SocketAddr>,
    ) -> io::Result<(Self, Option<(File, TcpListener)>)> {
        let config = root.join("address");
        let saved = match fs::read_to_string(&config) {
            Ok(text) => Some(
                text.trim()
                    .parse()
                    .map_err(|_| invalid("invalid saved dashboard address"))?,
            ),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        if let (Some(old), Some(new)) = (saved, desired)
            && new.port() != 0
            && new != old
        {
            return Err(invalid(
                "dashboard scope already has a different address; use its saved address or another scope",
            ));
        }
        let requested = saved
            .or(desired)
            .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 0)));
        if !requested.ip().is_loopback() {
            return Err(invalid(
                "dashboard requires loopback; use a tunnel or authenticated ingress for remote access",
            ));
        }
        let listener = Self::claim(&root, requested)?;
        let addr = match &listener {
            Some((_, socket)) => socket.local_addr()?,
            None => saved
                .ok_or_else(|| invalid("dashboard listener held without a published address"))?,
        };
        if saved.is_none() {
            atomic_write(&config, addr.to_string().as_bytes())?;
        }
        let token_file = root.join("viewer-token");
        if !token_file.try_exists()? {
            atomic_write(&token_file, random()?.as_bytes())?;
        }
        let token = http::private_text(&token_file)?;
        let token = token.trim();
        if token.len() < 32 {
            return Err(invalid("invalid dashboard viewer-token"));
        }
        let id = digest(root.as_os_str().as_encoded_bytes());
        Ok((
            Self {
                root,
                addr,
                token_hash: digest(token.as_bytes()),
                id,
            },
            listener,
        ))
    }

    fn claim(root: &Path, addr: SocketAddr) -> io::Result<Option<(File, TcpListener)>> {
        let lock = private_file(&root.join("listener.lock"))?;
        match lock.try_lock() {
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
            Ok(()) => {
                let listener = TcpListener::bind(addr).map_err(|e| {
                    io::Error::new(e.kind(), format!("cannot bind dashboard {addr}: {e}"))
                })?;
                listener.set_nonblocking(true)?;
                Ok(Some((lock, listener)))
            }
        }
    }
}

/// The observer and participating HTTP host for one live agent. Dropping the owner publishes
/// its last-known status and releases the listener so another registered host can take over.
pub struct Dashboard {
    agent: Arc<store::Agent>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Dashboard {
    /// Register a host after its agent state lease is held.
    ///
    /// # Errors
    /// The private registry, token, listener or agent record cannot be initialized.
    pub fn start(
        state: &Path,
        spec: &AgentSpec,
        revision: &str,
        model: Option<&str>,
    ) -> Result<Self, String> {
        Self::open(state, spec, revision, model)
            .map_err(|e| format!("agent dashboard startup failed: {e}"))
    }

    fn open(
        state: &Path,
        spec: &AgentSpec,
        revision: &str,
        model: Option<&str>,
    ) -> io::Result<Self> {
        let desired = std::env::var("LOOM_DASHBOARD_ADDR")
            .ok()
            .map(|s| {
                s.parse::<SocketAddr>()
                    .map_err(|_| invalid("LOOM_DASHBOARD_ADDR needs a loopback IP:port"))
            })
            .transpose()?;
        Self::in_scope(root()?, desired, state, spec, revision, model)
    }

    fn in_scope(
        root: PathBuf,
        desired: Option<SocketAddr>,
        state: &Path,
        spec: &AgentSpec,
        revision: &str,
        model: Option<&str>,
    ) -> io::Result<Self> {
        private_dir(&root)?;
        let root = root.canonicalize()?;
        let setup = private_file(&root.join("startup.lock"))?;
        setup.lock()?;
        let (scope, mut listener) = Scope::open(root, desired)?;
        private_dir(&scope.root.join("agents"))?;
        let agent = Arc::new(store::Agent::register(
            &scope.root,
            state,
            spec,
            revision,
            model,
        )?);
        eprintln!(
            "b10x-loom: dashboard http://{}/ (viewer token file: {})",
            scope.addr,
            scope.root.join("viewer-token").display()
        );
        drop(setup);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let lease = agent.clone();
        let worker = thread::Builder::new()
            .name("loom-dashboard".into())
            .spawn(move || {
                let mut last_seen = Instant::now();
                let scope = Arc::new(scope);
                let clients = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                while !worker_stop.load(Ordering::Acquire) {
                    if listener.is_none() {
                        match Scope::claim(&scope.root, scope.addr) {
                            Ok(next) => listener = next,
                            Err(error) => {
                                eprintln!("b10x-loom: dashboard listener takeover: {error}");
                                thread::sleep(Duration::from_millis(500));
                            }
                        }
                    }
                    if let Some((_, socket)) = &listener {
                        http::accept(socket, &scope, &clients);
                    }
                    if last_seen.elapsed() >= Duration::from_secs(5) {
                        lease.heartbeat();
                        last_seen = Instant::now();
                    }
                    thread::sleep(Duration::from_millis(35));
                }
            })?;
        Ok(Self {
            agent,
            stop,
            worker: Some(worker),
        })
    }

    /// A read-only observer suitable for a plugin host or A2A listener.
    pub fn observer(&self) -> Arc<dyn TurnObserver> {
        self.agent.clone()
    }
}

impl Drop for Dashboard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.agent.offline();
    }
}

#[cfg(test)]
mod tests;
