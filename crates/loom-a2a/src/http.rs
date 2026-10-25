//! HTTP/1.1, as much as the listener needs: one request per connection, read in two parts (the
//! request line and headers first, so a request is admitted before its body is read, then exactly
//! `Content-Length` bytes), one answer, then the connection closes. Every part is bounded, in size
//! and in time as a whole, not only per read: the head must arrive within [`HEAD_DEADLINE`] of the
//! connection's start, the body within [`BODY_DEADLINE`] of the head's end, the answer be written
//! within [`WRITE_DEADLINE`], so a caller trickling a byte at a time cannot hold memory or a
//! connection without end.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// The most bytes of the request line and headers together.
pub const MAX_HEAD: usize = 16 * 1024;

/// The most headers kept from one request.
pub const MAX_HEADERS: usize = 64;

/// The largest body read, in bytes.
pub const MAX_BODY: usize = 1024 * 1024;

/// How long one read may wait for a byte.
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// How long writing the answer may stall.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the request line and headers may take as a whole, from the connection's start.
pub const HEAD_DEADLINE: Duration = Duration::from_secs(10);

/// How long the body may take as a whole, from the end of the head.
pub const BODY_DEADLINE: Duration = Duration::from_secs(30);

/// How long writing the answer may take as a whole.
pub const WRITE_DEADLINE: Duration = Duration::from_secs(10);

/// How long what the caller still sends after the answer is drained, as a whole.
pub const DRAIN_DEADLINE: Duration = Duration::from_secs(1);

/// The deadlines of one connection.
#[derive(Debug, Clone, Copy)]
struct Deadlines {
    head: Duration,
    body: Duration,
    write: Duration,
    drain: Duration,
}

const DEADLINES: Deadlines = Deadlines {
    head: HEAD_DEADLINE,
    body: BODY_DEADLINE,
    write: WRITE_DEADLINE,
    drain: DRAIN_DEADLINE,
};

/// A stream every read and write of which ends by `deadline`: each waits at most the time left,
/// and none starts once it passed.
struct Timed {
    stream: TcpStream,
    deadline: Instant,
}

impl Timed {
    /// The time left, or `TimedOut` once the deadline passed.
    fn left(&self) -> io::Result<Duration> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the deadline passed",
            ));
        }
        Ok(left)
    }
}

impl Read for Timed {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.left()?;
        self.stream.set_read_timeout(Some(left.min(READ_TIMEOUT)))?;
        self.stream.read(buf)
    }
}

impl Write for Timed {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let left = self.left()?;
        self.stream
            .set_write_timeout(Some(left.min(WRITE_TIMEOUT)))?;
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

/// The answer to a head or body that could not be read: `408` when its deadline passed.
fn unread(error: &io::Error, what: &str) -> Response {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        Response::error(408, &format!("the {what} took too long"))
    } else {
        Response::error(400, &format!("the {what} could not be read"))
    }
}

/// The request line and headers of one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The method, verbatim.
    pub method: String,
    /// The target without its query string.
    pub path: String,
    /// Every header kept, its name lower-cased and its value trimmed.
    pub headers: Vec<(String, String)>,
}

impl Head {
    /// The value of the first header `name` (lower case).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value.as_str())
    }
}

/// One answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// Headers beside `Content-Type`, `Content-Length` and `Connection`.
    pub headers: Vec<(&'static str, String)>,
    /// The JSON body.
    pub body: String,
}

impl Response {
    /// A JSON answer of `status` with `body`.
    pub fn json(status: u16, body: &serde_json::Value) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_string(),
        }
    }

    /// A JSON answer `{"error": <detail>}`.
    pub fn error(status: u16, detail: &str) -> Self {
        Self::json(status, &serde_json::json!({ "error": detail }))
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// A connection, read in two parts.
pub struct Connection {
    reader: BufReader<Timed>,
    deadlines: Deadlines,
}

impl Connection {
    /// The connection over `stream`, its head due within [`HEAD_DEADLINE`] from now.
    ///
    /// # Errors
    /// The stream cannot be made blocking.
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        Self::with_deadlines(stream, DEADLINES)
    }

    fn with_deadlines(stream: TcpStream, deadlines: Deadlines) -> io::Result<Self> {
        stream.set_nonblocking(false)?;
        Ok(Self {
            reader: BufReader::new(Timed {
                stream,
                deadline: Instant::now() + deadlines.head,
            }),
            deadlines,
        })
    }

    /// The request line and headers, or the answer a malformed, oversized or late head gets.
    ///
    /// # Errors
    /// The answer to send instead: `400` for a head that is no HTTP/1.1 request, `408` for one
    /// not read whole within [`HEAD_DEADLINE`], `431` past [`MAX_HEAD`] or [`MAX_HEADERS`].
    pub fn head(&mut self) -> Result<Head, Response> {
        let mut read = 0_usize;
        let mut line = |reader: &mut BufReader<Timed>| -> Result<String, Response> {
            let mut bytes = Vec::new();
            let limit = (MAX_HEAD - read.min(MAX_HEAD)) as u64 + 1;
            reader
                .by_ref()
                .take(limit)
                .read_until(b'\n', &mut bytes)
                .map_err(|error| unread(&error, "request head"))?;
            read += bytes.len();
            if read > MAX_HEAD {
                return Err(Response::error(431, "the request head is too large"));
            }
            if !bytes.ends_with(b"\n") {
                return Err(Response::error(400, "the request head ended early"));
            }
            String::from_utf8(bytes)
                .map(|text| text.trim_end_matches(['\r', '\n']).to_owned())
                .map_err(|_| Response::error(400, "the request head is not UTF-8"))
        };
        let request_line = line(&mut self.reader)?;
        let mut parts = request_line.split(' ');
        let (Some(method), Some(target), Some(version), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(Response::error(400, "the request line is malformed"));
        };
        if !version.starts_with("HTTP/1.") || method.is_empty() || !target.starts_with('/') {
            return Err(Response::error(400, "the request line is malformed"));
        }
        let path = target.split('?').next().unwrap_or(target).to_owned();
        let mut headers = Vec::new();
        loop {
            let header = line(&mut self.reader)?;
            if header.is_empty() {
                break;
            }
            if headers.len() == MAX_HEADERS {
                return Err(Response::error(431, "the request has too many headers"));
            }
            let Some((name, value)) = header.split_once(':') else {
                return Err(Response::error(400, "a header is malformed"));
            };
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
        Ok(Head {
            method: method.to_owned(),
            path,
            headers,
        })
    }

    /// The body `head` announces: exactly `Content-Length` bytes, none without the header, due
    /// within [`BODY_DEADLINE`] from now.
    ///
    /// # Errors
    /// The answer to send instead: `411` for a chunked body, `413` past [`MAX_BODY`], `400` for a
    /// length that is no number or a body that ends early, `408` for one not read whole in time.
    pub fn body(&mut self, head: &Head) -> Result<Vec<u8>, Response> {
        self.reader.get_mut().deadline = Instant::now() + self.deadlines.body;
        if head.header("transfer-encoding").is_some() {
            return Err(Response::error(411, "send the body with a Content-Length"));
        }
        let Some(length) = head.header("content-length") else {
            return Ok(Vec::new());
        };
        let length: usize = length
            .parse()
            .map_err(|_| Response::error(400, "Content-Length is no number"))?;
        if length > MAX_BODY {
            return Err(Response::error(413, "the body is too large"));
        }
        let mut body = vec![0; length];
        self.reader.read_exact(&mut body).map_err(|error| {
            if error.kind() == io::ErrorKind::UnexpectedEof {
                Response::error(400, "the body ended early")
            } else {
                unread(&error, "request body")
            }
        })?;
        Ok(body)
    }

    /// Writes `response` within [`WRITE_DEADLINE`] and closes the connection, draining what the
    /// caller still sends for at most [`DRAIN_DEADLINE`].
    pub fn answer(mut self, response: &Response) {
        let mut text = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            response.status,
            reason(response.status),
            response.body.len()
        );
        for (name, value) in &response.headers {
            text.push_str(&format!("{name}: {value}\r\n"));
        }
        text.push_str("\r\n");
        text.push_str(&response.body);
        let deadlines = self.deadlines;
        let timed = self.reader.get_mut();
        timed.deadline = Instant::now() + deadlines.write;
        let _ = timed.write_all(text.as_bytes());
        let _ = timed.flush();
        let _ = timed.stream.shutdown(std::net::Shutdown::Write);
        // Whatever the caller still sends is drained, briefly and for a bounded time as a whole,
        // so closing does not reset the connection before the caller has read the answer.
        timed.deadline = Instant::now() + deadlines.drain;
        let mut rest = [0_u8; 1024];
        let mut drained = 0;
        while drained < MAX_BODY {
            match timed.read(&mut rest) {
                Ok(0) | Err(_) => break,
                Ok(read) => drained += read,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    const SHORT: Deadlines = Deadlines {
        head: Duration::from_millis(400),
        body: Duration::from_millis(400),
        write: Duration::from_millis(400),
        drain: Duration::from_millis(200),
    };

    /// A connection with [`SHORT`] deadlines over a loopback pair, and the caller's end.
    fn pair() -> (Connection, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let caller = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();
        (Connection::with_deadlines(stream, SHORT).unwrap(), caller)
    }

    /// Writes `bytes` one at a time, `pause` apart, on another thread, until a write fails.
    fn trickle(mut caller: TcpStream, bytes: Vec<u8>, pause: Duration) {
        std::thread::spawn(move || {
            for byte in bytes {
                if caller.write_all(&[byte]).is_err() {
                    break;
                }
                std::thread::sleep(pause);
            }
        });
    }

    #[test]
    fn a_trickled_head_is_cut_off_at_its_deadline() {
        let (mut connection, caller) = pair();
        trickle(
            caller,
            format!("GET /live HTTP/1.1\r\nX-Pad: {}\r\n\r\n", "a".repeat(100)).into_bytes(),
            Duration::from_millis(50),
        );
        let started = Instant::now();
        let refused = connection.head().expect_err("the head is late");
        assert_eq!(refused.status, 408, "{refused:?}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_trickled_body_is_cut_off_at_its_deadline() {
        let (mut connection, caller) = pair();
        let body = "x".repeat(200);
        let mut request = format!(
            "POST /a2a HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        let head_len = request.len();
        request.extend_from_slice(body.as_bytes());
        let mut caller_head = caller.try_clone().unwrap();
        caller_head.write_all(&request[..head_len]).unwrap();
        trickle(
            caller,
            request[head_len..].to_vec(),
            Duration::from_millis(50),
        );
        let head = connection.head().expect("the head is whole");
        let started = Instant::now();
        let refused = connection.body(&head).expect_err("the body is late");
        assert_eq!(refused.status, 408, "{refused:?}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn draining_a_trickling_caller_ends_at_its_deadline() {
        let (connection, caller) = pair();
        trickle(caller, vec![b'a'; 400], Duration::from_millis(20));
        let started = Instant::now();
        connection.answer(&Response::error(408, "late"));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }
}
