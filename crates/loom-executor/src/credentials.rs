// SPDX-License-Identifier: Apache-2.0

//! A wire's credential taken from the reference a run configuration names.
//!
//! The ported wires take a [`BearerSource`] from their embedder. [`ResolvedBearer`] is the one
//! that resolves a [`WireCredential`] — a `loom.run.CredentialReference` and its
//! `loom.run.CredentialKind`, generated from `ess/domains/run.yaml` — through llm's
//! `b10x-llm-credentials` [`SecretResolver`], injected by the embedder. A run can then use an
//! OAuth subscription login with no secret value in any configuration.
//!
//! It lives beside `harness`, not in `harness::wire`: the wire module performs no I/O and names
//! no runtime, and this source drives a resolver's future on a runtime of its own.

use std::sync::Arc;
use std::time::Duration;

use llm_credentials::{SecretRef, SecretResolver};
use serde::Serialize;
use zeroize::Zeroizing;

use crate::harness::wire::{Bearer, BearerSource, CredentialKind, WireError};
use crate::model::run::{CredentialReference, WireCredential};

/// A credential resolved per call from the reference a run configuration names
/// ([`WireCredential`]), by a [`SecretResolver`] the embedder injects.
///
/// The configuration holds the reference and its kind; the resolver holds the secret. Loom code
/// never reads a credential file: a Codex login, a keychain or an environment variable is the
/// resolver's business (llm's `b10x-llm-credentials` adapters, chosen by the embedder).
///
/// [`BearerSource::bearer`] is synchronous and a resolver answers with a future. Each call drives
/// that future on a short-lived thread with its own current-thread runtime, so it never nests a
/// runtime and can be called from any thread. It **blocks the calling thread** until the resolver
/// answers or [`RESOLVE_BOUND`] passes, and at the bound it refuses the call naming the reference.
/// Called from an async task, it blocks that task's worker thread for that time: on a
/// current-thread runtime nothing else on that runtime runs meanwhile, so a resolver that waits
/// for work scheduled on the caller's runtime does not finish and is refused at the bound. Call
/// the wire from a blocking context (`tokio::task::spawn_blocking`) when the resolver needs the
/// caller's runtime. The bound applies at the resolver's `.await` points; a resolver that blocks
/// its thread without yielding is not interrupted.
///
/// A resolved secret that is empty, is not UTF-8 or contains a control character (CR, LF, NUL,
/// tab, …) is refused at the source, before any wire sees it: no header can carry it. llm's file
/// and environment adapters return the stored bytes untrimmed, so a key file written with `echo`
/// is refused for its trailing newline. Every refusal is `Unauthorized`, not retriable, and names
/// the reference, never the value.
///
/// Renewal of a rejected token ([`SecretResolver::refresh`]) has no hook here.
pub struct ResolvedBearer {
    reference: SecretRef,
    kind: CredentialKind,
    resolver: Arc<dyn SecretResolver>,
    bound: Duration,
}

/// How long one [`ResolvedBearer::bearer`] call waits for its resolver before refusing.
pub const RESOLVE_BOUND: Duration = Duration::from_secs(30);

impl ResolvedBearer {
    /// Builds a source for one configured wire credential.
    ///
    /// # Errors
    ///
    /// Returns [`WireErrorCode::Unauthorized`](crate::harness::wire::WireErrorCode) when the
    /// reference is not one a resolver could look up (empty, oversized or non-printable).
    pub fn new(
        credential: &WireCredential,
        resolver: Arc<dyn SecretResolver>,
    ) -> Result<Self, WireError> {
        let name = &credential.reference.0;
        let reference = SecretRef::new(name.as_str()).map_err(|refusal| {
            WireError::unauthorized(format!("credential reference `{name}` refused: {refusal}"))
        })?;
        Ok(Self {
            reference,
            kind: credential.kind,
            resolver,
            bound: RESOLVE_BOUND,
        })
    }

    fn refused(&self, why: impl std::fmt::Display) -> WireError {
        WireError::unauthorized(format!(
            "credential reference `{}` refused: {why}",
            self.reference.as_str()
        ))
    }
}

impl std::fmt::Debug for ResolvedBearer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedBearer")
            .field("reference", &self.reference.as_str())
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl BearerSource for ResolvedBearer {
    fn bearer(&self) -> Result<Bearer, WireError> {
        let resolved = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|_| self.refused("no runtime to resolve it on"))?;
                    runtime
                        .block_on(async {
                            tokio::time::timeout(self.bound, self.resolver.resolve(&self.reference))
                                .await
                        })
                        .map_err(|_| self.refused("the resolver did not answer within its bound"))?
                        .map_err(|refusal| self.refused(refusal))
                })
                .join()
                .unwrap_or_else(|_| Err(self.refused("the resolver panicked")))
        })?;
        let bytes = resolved.secret.expose().to_vec();
        drop(resolved);
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(not_text) => {
                drop(Zeroizing::new(not_text.into_bytes()));
                return Err(self.refused("the secret is not text"));
            }
        };
        let unpresentable = if text.is_empty() {
            Some("the secret is empty")
        } else if text.chars().any(char::is_control) {
            Some("the secret contains a control character, which no header can carry")
        } else {
            None
        };
        match unpresentable {
            None => Ok(Bearer::new(text)),
            Some(why) => {
                drop(Zeroizing::new(text));
                Err(self.refused(why))
            }
        }
    }

    fn kind(&self) -> CredentialKind {
        self.kind
    }
}

/// [`WireCredential`] as it is written in a run configuration: the reference and the kind, and
/// nothing a secret could be written into.
#[derive(Serialize)]
struct CredentialEntry {
    reference: String,
    kind: KindEntry,
}

/// [`CredentialKind`] as it is written in a run configuration.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum KindEntry {
    ApiKey,
    Oauth,
}

const REFERENCE_FIELD: &str = "reference";
const KIND_FIELD: &str = "kind";

/// Encodes one wire credential for a run configuration.
pub fn encode_wire_credential(credential: &WireCredential) -> serde_json::Value {
    let entry = CredentialEntry {
        reference: credential.reference.0.clone(),
        kind: match credential.kind {
            CredentialKind::ApiKey => KindEntry::ApiKey,
            CredentialKind::Oauth => KindEntry::Oauth,
        },
    };
    serde_json::to_value(entry).expect("a reference and a kind always encode")
}

/// Decodes one wire credential from a run configuration.
///
/// # Errors
///
/// Returns [`WireErrorCode::Protocol`](crate::harness::wire::WireErrorCode) for a value that is
/// not exactly a reference and a kind. The error names the field at fault and never echoes a
/// value from the input, so a secret pasted into the wrong field does not reach a log.
pub fn decode_wire_credential(value: &serde_json::Value) -> Result<WireCredential, WireError> {
    let refused = |why: &str| WireError::protocol(format!("not a wire credential: {why}"));
    let entry = value.as_object().ok_or_else(|| refused("not an object"))?;
    if entry
        .keys()
        .any(|field| field != REFERENCE_FIELD && field != KIND_FIELD)
    {
        return Err(refused(
            "it holds a field other than `reference` and `kind`",
        ));
    }
    let reference = entry
        .get(REFERENCE_FIELD)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| refused("field `reference` is missing or not a string"))?;
    let kind = match entry.get(KIND_FIELD).and_then(serde_json::Value::as_str) {
        Some("api-key") => CredentialKind::ApiKey,
        Some("oauth") => CredentialKind::Oauth,
        _ => return Err(refused("field `kind` is not `api-key` or `oauth`")),
    };
    Ok(WireCredential {
        reference: CredentialReference(reference.to_owned()),
        kind,
    })
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;

    use llm_credentials::{ResolvedSecret, SecretError};

    use super::*;
    use crate::harness::wire::WireErrorCode;

    /// Never answers.
    struct Silent;

    impl SecretResolver for Silent {
        fn resolve<'a>(
            &'a self,
            _reference: &'a SecretRef,
        ) -> Pin<Box<dyn Future<Output = Result<ResolvedSecret, SecretError>> + Send + 'a>>
        {
            Box::pin(std::future::pending())
        }
    }

    #[test]
    fn a_resolver_that_never_answers_is_refused_at_the_bound_naming_the_reference() {
        let credential = WireCredential {
            reference: CredentialReference("silent-login".to_owned()),
            kind: CredentialKind::Oauth,
        };
        let mut bearer = ResolvedBearer::new(&credential, Arc::new(Silent)).expect("valid");
        bearer.bound = Duration::from_millis(50);

        let error = bearer.bearer().expect_err("refused at the bound");

        assert_eq!(error.code, WireErrorCode::Unauthorized, "{error:?}");
        assert!(!error.retriable, "{error:?}");
        assert!(error.message.contains("silent-login"), "{error:?}");
        assert!(error.message.contains("bound"), "{error:?}");
    }

    #[test]
    fn every_decode_refusal_names_a_field_and_echoes_no_value() {
        let value = "SECRET-pasted-value";
        for input in [
            serde_json::json!(value),
            serde_json::json!({"reference": "login", "kind": value}),
            serde_json::json!({"reference": value}),
            serde_json::json!({"reference": 7, "kind": "oauth"}),
            serde_json::json!({"reference": "login", "kind": "oauth", value: value}),
        ] {
            let error = decode_wire_credential(&input).expect_err("not a wire credential");
            assert_eq!(error.code, WireErrorCode::Protocol, "{error:?}");
            assert!(!error.message.contains(value), "{error:?}");
        }
    }
}
