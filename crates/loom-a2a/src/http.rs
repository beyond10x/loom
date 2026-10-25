//! HTTP/1.1, as much as the listener needs: one request per connection, read in two parts (the
//! request line and headers first, so a request is admitted before its body is read, then exactly
//! `Content-Length` bytes), one answer, then the connection closes. Every part is bounded, so a
//! caller cannot hold memory or a connection without end.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

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
    reader: BufReader<TcpStream>,
}

impl Connection {
    /// The connection over `stream`, with the read and write bounds set.
    ///
    /// # Errors
    /// The timeouts cannot be set.
    pub fn new(stream: TcpStream) -> std::io::Result<Self> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        Ok(Self {
            reader: BufReader::new(stream),
        })
    }

    /// The request line and headers, or the answer a malformed or oversized head gets.
    ///
    /// # Errors
    /// The answer to send instead: `400` for a head that is no HTTP/1.1 request, `431` past
    /// [`MAX_HEAD`] or [`MAX_HEADERS`].
    pub fn head(&mut self) -> Result<Head, Response> {
        let mut read = 0_usize;
        let mut line = |reader: &mut BufReader<TcpStream>| -> Result<String, Response> {
            let mut bytes = Vec::new();
            let limit = (MAX_HEAD - read.min(MAX_HEAD)) as u64 + 1;
            reader
                .by_ref()
                .take(limit)
                .read_until(b'\n', &mut bytes)
                .map_err(|_| Response::error(400, "the request could not be read"))?;
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

    /// The body `head` announces: exactly `Content-Length` bytes, none without the header.
    ///
    /// # Errors
    /// The answer to send instead: `411` for a chunked body, `413` past [`MAX_BODY`], `400` for a
    /// length that is no number or a body that ends early.
    pub fn body(&mut self, head: &Head) -> Result<Vec<u8>, Response> {
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
        self.reader
            .read_exact(&mut body)
            .map_err(|_| Response::error(400, "the body ended early"))?;
        Ok(body)
    }

    /// Writes `response` and closes the connection.
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
        let stream = self.reader.get_mut();
        let _ = stream.write_all(text.as_bytes());
        let _ = stream.flush();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        // Whatever the caller still sends is drained, briefly, so closing does not reset the
        // connection before the caller has read the answer.
        let _ = self
            .reader
            .get_mut()
            .set_read_timeout(Some(Duration::from_secs(1)));
        let mut rest = [0_u8; 1024];
        let mut drained = 0;
        while drained < MAX_BODY {
            match self.reader.get_mut().read(&mut rest) {
                Ok(0) | Err(_) => break,
                Ok(read) => drained += read,
            }
        }
    }
}
