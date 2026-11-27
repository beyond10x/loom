//! Same-user IPC to the agent generation that owns a pinned, authenticated A2A listener.
use super::{Scope, digest, http, invalid, store::is_digest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, TcpListener, TcpStream},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use subtle::ConstantTimeEq;

const TURN_WAIT: Duration = Duration::from_secs(55);
const MAX_TEXT: usize = 16_384;
const MAX_FRAME: u64 = 128 * 1024;
const MAX_A2A_RESPONSE: u64 = 8 * 1024 * 1024;

/// A bound A2A socket of this process. The clone pins its address for *every* outstanding relay.
pub struct A2aIngress {
    listener: TcpListener,
    path: String,
    bearer: String,
}

impl A2aIngress {
    /// Capture the actual listener and the same path and credential used by the A2A host.
    /// # Errors
    /// The descriptor cannot be cloned, or the path or header would not be a valid HTTP value.
    pub fn new(listener: &TcpListener, path: &str, bearer: &str) -> Result<Self, String> {
        let bearer = bearer.trim();
        if !path.starts_with('/')
            || path
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
            || bearer.is_empty()
            || bearer.chars().any(char::is_control)
        {
            return Err("invalid A2A dispatch configuration".into());
        }
        Ok(Self {
            listener: listener.try_clone().map_err(|e| e.to_string())?,
            path: path.into(),
            bearer: bearer.into(),
        })
    }

    fn turn(&self, message: &Message) -> io::Result<Outcome> {
        let mut addr = self.listener.local_addr()?;
        if addr.ip().is_unspecified() {
            addr.set_ip(if addr.is_ipv4() {
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            } else {
                IpAddr::V6(Ipv6Addr::LOCALHOST)
            });
        }
        let mut conn = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
        conn.set_write_timeout(Some(Duration::from_secs(3)))?;
        conn.set_read_timeout(Some(TURN_WAIT))?;
        let body = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":"SendMessage",
            "params":{"message":{"messageId":message.message_id,"role":"ROLE_USER","parts":[{"text":message.text}]}}}))?;
        write!(
            conn,
            "POST {} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.path,
            addr,
            self.bearer,
            body.len()
        )?;
        conn.write_all(&body)?;
        conn.shutdown(Shutdown::Write)?;
        // The existing A2A HTTP server uses Content-Length and closes the connection per request.
        let mut response = Vec::new();
        conn.take(MAX_A2A_RESPONSE + 1).read_to_end(&mut response)?;
        if response.len() as u64 > MAX_A2A_RESPONSE {
            return Err(invalid("A2A response exceeds bound"));
        }
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut head = httparse::Response::new(&mut headers);
        let Ok(httparse::Status::Complete(offset)) = head.parse(&response) else {
            return Err(invalid("invalid A2A response"));
        };
        if head.code != Some(200) {
            return Ok(Outcome::Unavailable);
        }
        let envelope: Value = serde_json::from_slice(&response[offset..])?;
        let state = envelope["result"]["task"]["status"]["state"]
            .as_str()
            .unwrap_or_default();
        Ok(match state {
            "TASK_STATE_COMPLETED" | "TASK_STATE_FAILED" | "TASK_STATE_REJECTED" => {
                Outcome::Ended(state.to_owned())
            }
            _ => Outcome::Unavailable,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    message_id: String,
    text: String,
}

impl Message {
    fn valid(&self) -> bool {
        (1..=128).contains(&self.message_id.len())
            && self
                .message_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            && self.text.len() <= MAX_TEXT
            && !self.text.trim().is_empty()
    }
}

#[derive(Serialize, Deserialize)]
struct Request {
    generation: String,
    proof: String,
    message: Option<Message>,
}

#[derive(Serialize, Deserialize)]
enum Outcome {
    Authenticated,
    Denied,
    Offline,
    Unavailable,
    Ended(String),
}

fn proof(generation: &str, bearer: &str) -> String {
    digest(format!("{generation}\0{bearer}").as_bytes())
}

/// The Unix socket lives in the scope's 0700 directory. A directory handle keeps long state
/// paths out of Linux's 108-byte socket-address field without introducing a public rendezvous.
fn socket_path(directory: &Path, id: &str) -> io::Result<(File, PathBuf)> {
    if !is_digest(id) {
        return Err(invalid("invalid agent id"));
    }
    let dir = File::open(directory)?;
    let filename = format!("{id}.sock");
    let path = directory.join(&filename);
    #[cfg(target_os = "linux")]
    let path = if path.as_os_str().as_encoded_bytes().len() >= 108 {
        use std::os::fd::AsRawFd;
        PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd())).join(filename)
    } else {
        path
    };
    Ok((dir, path))
}

pub(super) struct Bridge {
    socket: UnixListener,
    _directory: File,
    path: PathBuf,
    ingress: Arc<A2aIngress>,
    workers: Arc<AtomicUsize>,
}

impl Bridge {
    pub fn bind(directory: &Path, id: &str, ingress: A2aIngress) -> io::Result<Self> {
        let (dir, path) = socket_path(directory, id)?;
        // The caller holds both this agent's exclusive state lease and the scope startup lock.
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            return Err(error);
        }
        let socket = UnixListener::bind(&path)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            _directory: dir,
            path,
            ingress: Arc::new(ingress),
            workers: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn accept(&self, instance: &str) {
        let Ok((mut socket, _)) = self.socket.accept() else {
            return;
        };
        if self.workers.load(Ordering::Acquire) >= 4 {
            let _ = serde_json::to_writer(socket, &Outcome::Unavailable);
            return;
        }
        self.workers.fetch_add(1, Ordering::AcqRel);
        let workers = self.workers.clone();
        let ingress = self.ingress.clone();
        let instance = instance.to_owned();
        let started = std::thread::Builder::new()
            .name("loom-agent-dispatch".into())
            .spawn(move || {
                struct Active(Arc<AtomicUsize>);
                impl Drop for Active {
                    fn drop(&mut self) {
                        self.0.fetch_sub(1, Ordering::Release);
                    }
                }
                let _active = Active(workers);
                let handle = |socket: &mut UnixStream| -> io::Result<Outcome> {
                    socket.set_read_timeout(Some(Duration::from_secs(3)))?;
                    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
                    let mut incoming = Vec::new();
                    socket.take(MAX_FRAME + 1).read_to_end(&mut incoming)?;
                    if incoming.len() as u64 > MAX_FRAME {
                        return Ok(Outcome::Denied);
                    }
                    let request: Request = serde_json::from_slice(&incoming)?;
                    if request.generation != instance {
                        return Ok(Outcome::Offline);
                    }
                    let expected = proof(&instance, &ingress.bearer);
                    if !bool::from(expected.as_bytes().ct_eq(request.proof.as_bytes())) {
                        return Ok(Outcome::Denied);
                    }
                    match request.message {
                        None => Ok(Outcome::Authenticated),
                        Some(message) if message.valid() => ingress.turn(&message),
                        Some(_) => Ok(Outcome::Denied),
                    }
                };
                let outcome = handle(&mut socket).unwrap_or(Outcome::Unavailable);
                let _ = serde_json::to_writer(socket, &outcome);
            });
        if started.is_err() {
            self.workers.fetch_sub(1, Ordering::Release);
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn exchange(directory: &Path, id: &str, request: &Request) -> io::Result<Outcome> {
    let (_dir, path) = socket_path(directory, id)?;
    let mut socket = match UnixStream::connect(path) {
        Ok(s) => s,
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(Outcome::Offline);
        }
        Err(e) => return Err(e),
    };
    socket.set_read_timeout(Some(TURN_WAIT + Duration::from_secs(2)))?;
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    serde_json::to_writer(&mut socket, request)?;
    socket.shutdown(Shutdown::Write)?;
    Ok(serde_json::from_reader(socket.take(1024))?)
}

/// Verify a manually started host owns the template's bearer without submitting work.
pub(super) fn authenticates(
    scope: &Scope,
    id: &str,
    generation: &str,
    bearer: &str,
) -> io::Result<bool> {
    let request = Request {
        proof: proof(generation, bearer),
        generation: generation.into(),
        message: None,
    };
    match exchange(&scope.root.join("agents"), id, &request)? {
        Outcome::Authenticated => Ok(true),
        Outcome::Offline | Outcome::Denied => Ok(false),
        _ => Err(invalid("agent credential check unavailable")),
    }
}

pub(super) fn handle(
    scope: &Scope,
    id: &str,
    bearer: &str,
    bytes: &[u8],
    stream: &mut TcpStream,
) -> io::Result<()> {
    let result = |stream: &mut TcpStream| -> io::Result<()> {
        let Some(row) = http::registration(scope, id)? else {
            return http::error(stream, "404 Not Found", "not_found");
        };
        if !row.online || row.a2a_protocol.is_none() {
            return http::error(stream, "409 Conflict", "agent_unavailable");
        }
        if bearer.trim().is_empty() {
            return http::error(stream, "403 Forbidden", "agent_unauthorized");
        }
        let Ok(message) = serde_json::from_slice::<Message>(bytes) else {
            return http::error(stream, "400 Bad Request", "invalid_request");
        };
        if !message.valid() {
            return http::error(stream, "400 Bad Request", "invalid_request");
        }
        let revision = b10x_loom_a2a::item_revision(&message.message_id);
        let run_id = digest(format!("{id}\0a2a:{revision}").as_bytes());
        let request = Request {
            proof: proof(&row.instance, bearer.trim()),
            generation: row.instance,
            message: Some(message),
        };
        match exchange(&scope.root.join("agents"), id, &request)? {
            Outcome::Ended(state) => http::respond(
                stream,
                "200 OK",
                "application/json",
                &serde_json::to_vec(&json!({"run_id":run_id,"state":state}))?,
                "",
            ),
            Outcome::Denied => http::error(stream, "403 Forbidden", "agent_unauthorized"),
            Outcome::Offline => http::error(stream, "409 Conflict", "agent_unavailable"),
            Outcome::Authenticated | Outcome::Unavailable => {
                http::error(stream, "503 Service Unavailable", "dispatch_unavailable")
            }
        }
    };
    result(stream)
        .or_else(|_| http::error(stream, "503 Service Unavailable", "dispatch_unavailable"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_generation_and_wrong_proof_never_reach_the_pinned_agent_port() {
        let root = tempfile::tempdir().unwrap();
        let original = TcpListener::bind("127.0.0.1:0").unwrap();
        original.set_nonblocking(true).unwrap();
        let address = original.local_addr().unwrap();
        let bound = A2aIngress::new(&original, "/a2a", "real-token").unwrap();
        let id = digest(b"state");
        let bridge = Bridge::bind(root.path(), &id, bound).unwrap();
        let (directory, path) = socket_path(root.path(), &id).unwrap();
        let request = |generation: &str, bearer: &str| Request {
            generation: generation.into(),
            proof: proof(generation, bearer),
            message: Some(Message {
                message_id: "stable".into(),
                text: "example".into(),
            }),
        };
        let ask = |request: &Request| {
            let mut socket = UnixStream::connect(&path).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            serde_json::to_writer(&mut socket, request).unwrap();
            socket.shutdown(Shutdown::Write).unwrap();
            bridge.accept("current");
            serde_json::from_reader::<_, Outcome>(socket).unwrap()
        };
        assert!(matches!(
            ask(&request("previous", "real-token")),
            Outcome::Offline
        ));
        assert!(matches!(ask(&request("current", "wrong")), Outcome::Denied));
        assert_eq!(
            original.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(original);
        assert!(
            TcpListener::bind(address).is_err(),
            "the bridge still owns the original port"
        );
        drop(bridge);
        let replacement = TcpListener::bind(address).unwrap();
        replacement.set_nonblocking(true).unwrap();
        let stale = exchange(root.path(), &id, &request("current", "real-token")).unwrap();
        assert!(matches!(stale, Outcome::Offline));
        assert_eq!(
            replacement.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(directory);
    }
}
