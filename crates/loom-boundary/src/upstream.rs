//! An upstream: the plain `http://` base URL a relay forwards to.

use std::fmt;
use std::net::Ipv6Addr;

use hyper::header::HeaderValue;

/// A plain `http://` upstream: a host, a port and a base path, with no user information, query
/// or fragment. In-cluster TLS is not supported, so an `https` URL is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    /// The host the relay connects to: a name or an address, an IPv6 address without brackets.
    connect_host: String,
    port: u16,
    /// The `Host` header the upstream receives: the authority as written.
    authority: String,
    host_header: HeaderValue,
    /// The base path without a trailing slash; empty for none.
    base: String,
}

/// Why an upstream URL was refused. The message never repeats the URL, which could carry a
/// credential (user information, a query).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamError {
    /// The URL is not `http://…`.
    NotHttp,
    /// The URL is `https://…`; the relay speaks plain HTTP to in-cluster upstreams only.
    Https,
    /// The URL carries user information (`user:password@`).
    UserInfo,
    /// The URL carries a query.
    Query,
    /// The URL carries a fragment.
    Fragment,
    /// The URL names no valid host.
    Host,
    /// The URL names a port outside 1 to 65535, or one that is not a number.
    Port,
    /// The base path is not plain segments of letters, digits and `-._~`.
    Path,
}

impl fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UpstreamError::NotHttp => "must be a plain http:// URL",
            UpstreamError::Https => {
                "must be a plain http:// URL: an https upstream is not supported (in-cluster TLS is not implemented)"
            }
            UpstreamError::UserInfo => "must not carry user information",
            UpstreamError::Query => "must not carry a query",
            UpstreamError::Fragment => "must not carry a fragment",
            UpstreamError::Host => "names no valid host",
            UpstreamError::Port => "names no valid port (1 to 65535)",
            UpstreamError::Path => {
                "has a base path that is not plain segments of letters, digits and -._~"
            }
        })
    }
}

impl std::error::Error for UpstreamError {}

impl std::str::FromStr for Upstream {
    type Err = UpstreamError;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        Upstream::parse(url)
    }
}

impl Upstream {
    /// Parses `http://<host>[:<port>][/<base path>]`.
    pub fn parse(url: &str) -> Result<Self, UpstreamError> {
        let (scheme, rest) = url.split_once("://").ok_or(UpstreamError::NotHttp)?;
        if scheme.eq_ignore_ascii_case("https") {
            return Err(UpstreamError::Https);
        }
        if !scheme.eq_ignore_ascii_case("http") {
            return Err(UpstreamError::NotHttp);
        }
        if rest.contains('#') {
            return Err(UpstreamError::Fragment);
        }
        if rest.contains('?') {
            return Err(UpstreamError::Query);
        }
        let (authority, path) = match rest.find('/') {
            Some(at) => rest.split_at(at),
            None => (rest, ""),
        };
        if authority.contains('@') {
            return Err(UpstreamError::UserInfo);
        }
        let (connect_host, port) = host_and_port(authority)?;
        let authority = authority.to_ascii_lowercase();
        let host_header = HeaderValue::from_str(&authority).map_err(|_| UpstreamError::Host)?;
        Ok(Upstream {
            connect_host,
            port,
            authority,
            host_header,
            base: base_path(path)?,
        })
    }

    /// The `Host` header the upstream receives.
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// The base path every forwarded path is appended to, without a trailing slash.
    pub fn base_path(&self) -> &str {
        &self.base
    }

    pub(crate) fn host_header(&self) -> &HeaderValue {
        &self.host_header
    }

    pub(crate) fn connect_host(&self) -> &str {
        &self.connect_host
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }
}

fn host_and_port(authority: &str) -> Result<(String, u16), UpstreamError> {
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let (address, after) = bracketed.split_once(']').ok_or(UpstreamError::Host)?;
        address
            .parse::<Ipv6Addr>()
            .map_err(|_| UpstreamError::Host)?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':').ok_or(UpstreamError::Port)?),
        };
        (address.to_owned(), port)
    } else {
        let (host, port) = match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        };
        let name = |byte: &u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-');
        if host.is_empty()
            || !host.bytes().all(|byte| name(&byte))
            || host.bytes().all(|byte| byte == b'.')
        {
            return Err(UpstreamError::Host);
        }
        (host.to_ascii_lowercase(), port)
    };
    let port = match port {
        None => 80,
        Some(digits) => {
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(UpstreamError::Port);
            }
            match digits.parse::<u16>() {
                Ok(0) | Err(_) => return Err(UpstreamError::Port),
                Ok(port) => port,
            }
        }
    };
    Ok((host, port))
}

fn base_path(path: &str) -> Result<String, UpstreamError> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let plain =
        |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
    for segment in trimmed[1..].split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || !segment.bytes().all(plain) {
            return Err(UpstreamError::Path);
        }
    }
    Ok(trimmed.to_owned())
}
