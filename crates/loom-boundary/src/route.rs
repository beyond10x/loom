//! What each relay admits, decided from the method and the request target alone, before any
//! token is read or any byte is sent upstream.

use std::collections::BTreeSet;

use hyper::{Method, StatusCode, Uri};

use crate::Service;

/// A request a relay forwards: the target below the upstream's base path, and how.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Forward {
    /// The path and query appended to the upstream's base path.
    pub(crate) target: String,
    /// The request headers the upstream may receive.
    pub(crate) headers: Headers,
    /// Whether the body is a Connectors invoke, buffered and admitted before it is sent.
    pub(crate) invoke: bool,
}

/// Which of the agent's request headers pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Headers {
    /// Every header but credentials, hop-by-hop headers and framing.
    AllButDropped,
    /// Only the git smart-HTTP headers.
    Git,
}

/// A request refused locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Refusal {
    pub(crate) status: StatusCode,
    pub(crate) error: &'static str,
    pub(crate) message: &'static str,
}

const NOT_FOUND: Refusal = Refusal {
    status: StatusCode::NOT_FOUND,
    error: "not_relayed",
    message: "this boundary relays no such request",
};

const BAD_PATH: Refusal = Refusal {
    status: StatusCode::BAD_REQUEST,
    error: "path_refused",
    message: "the path has an empty, '.' or '..' segment, a backslash, or an encoded '.', '/' or '\\'",
};

const PROXY_FORM: Refusal = Refusal {
    status: StatusCode::BAD_REQUEST,
    error: "target_refused",
    message: "the request target must be a path; this boundary is not a proxy",
};

const METHOD: Refusal = Refusal {
    status: StatusCode::METHOD_NOT_ALLOWED,
    error: "method_refused",
    message: "CONNECT and TRACE are not relayed",
};

/// Decides `method` on `uri` for `service`.
pub(crate) fn route(service: &Service, method: &Method, uri: &Uri) -> Result<Forward, Refusal> {
    // TRACE would have the upstream echo the request, the token with it.
    if method == Method::CONNECT || method == Method::TRACE || method.as_str() == "TRACK" {
        return Err(METHOD);
    }
    if uri.scheme().is_some() || uri.authority().is_some() || !uri.path().starts_with('/') {
        return Err(PROXY_FORM);
    }
    match service {
        Service::Model => model(uri),
        Service::Connectors { .. } => connectors(method, uri),
        Service::Git { repositories } => git(method, uri, repositories),
    }
}

/// Any method on a path under `/v1/`, its query kept.
fn model(uri: &Uri) -> Result<Forward, Refusal> {
    let path = uri.path();
    if !path.starts_with("/v1/") {
        return Err(NOT_FOUND);
    }
    let lowered = path.to_ascii_lowercase();
    if path.contains('\\')
        || ["%2e", "%2f", "%5c"]
            .iter()
            .any(|encoded| lowered.contains(encoded))
        || path[1..]
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(BAD_PATH);
    }
    let target = match uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_owned(),
    };
    Ok(Forward {
        target,
        headers: Headers::AllButDropped,
        invoke: false,
    })
}

/// `GET /v1/describe` and `POST /v1alpha2/invoke`, each exactly, with no query.
fn connectors(method: &Method, uri: &Uri) -> Result<Forward, Refusal> {
    let target = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("");
    let invoke = match (method, target) {
        (&Method::GET, "/v1/describe") => false,
        (&Method::POST, "/v1alpha2/invoke") => true,
        _ => return Err(NOT_FOUND),
    };
    Ok(Forward {
        target: target.to_owned(),
        headers: Headers::AllButDropped,
        invoke,
    })
}

/// The three smart-HTTP routes of a declared `<owner>/<repo>`.
fn git(method: &Method, uri: &Uri, repositories: &BTreeSet<String>) -> Result<Forward, Refusal> {
    let segments: Vec<&str> = uri.path()[1..].split('/').collect();
    let query = uri.query();
    let (owner, repository, rest) = match (method, segments.as_slice(), query) {
        (
            &Method::GET,
            [owner, repository, "info", "refs"],
            Some("service=git-upload-pack" | "service=git-receive-pack"),
        ) => (*owner, *repository, "info/refs"),
        (
            &Method::POST,
            [
                owner,
                repository,
                rest @ ("git-upload-pack" | "git-receive-pack"),
            ],
            None,
        ) => (*owner, *repository, *rest),
        _ => return Err(NOT_FOUND),
    };
    let repository = repository.strip_suffix(".git").ok_or(NOT_FOUND)?;
    if !repository_part(owner) || !repository_part(repository) {
        return Err(NOT_FOUND);
    }
    let declared = format!("{owner}/{repository}");
    if !repositories.contains(&declared) {
        return Err(NOT_FOUND);
    }
    let target = match query {
        Some(query) => format!("/{declared}.git/{rest}?{query}"),
        None => format!("/{declared}.git/{rest}"),
    };
    Ok(Forward {
        target,
        headers: Headers::Git,
        invoke: false,
    })
}

/// One part of `<owner>/<repo>`: `[A-Za-z0-9._-]+`, neither `.` nor `..`.
pub(crate) fn repository_part(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
