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

use llm_credentials::{SecretRef, SecretResolver};
use serde::{Deserialize, Serialize};
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
/// that future to completion on a short-lived thread with its own current-thread runtime, so it
/// works the same on a plain thread and on a thread inside a running Tokio runtime: it never nests
/// a runtime and never blocks one it does not own. A wire call is already blocking, so the call
/// waits for the thread, as it waits for the response.
///
/// Renewal of a rejected token ([`SecretResolver::refresh`]) has no hook here.
pub struct ResolvedBearer {
    reference: SecretRef,
    kind: CredentialKind,
    resolver: Arc<dyn SecretResolver>,
}

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
                        .block_on(self.resolver.resolve(&self.reference))
                        .map_err(|refusal| self.refused(refusal))
                })
                .join()
                .unwrap_or_else(|_| Err(self.refused("the resolver panicked")))
        })?;
        let bytes = resolved.secret.expose().to_vec();
        drop(resolved);
        match String::from_utf8(bytes) {
            Ok(text) => Ok(Bearer::new(text)),
            Err(not_text) => {
                drop(Zeroizing::new(not_text.into_bytes()));
                Err(self.refused("the secret is not text"))
            }
        }
    }

    fn kind(&self) -> CredentialKind {
        self.kind
    }
}

/// [`WireCredential`] as it is written in a run configuration: the reference and the kind, and
/// nothing a secret could be written into.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialEntry {
    reference: String,
    kind: KindEntry,
}

/// [`CredentialKind`] as it is written in a run configuration.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum KindEntry {
    ApiKey,
    Oauth,
}

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
/// not exactly a reference and a kind.
pub fn decode_wire_credential(value: &serde_json::Value) -> Result<WireCredential, WireError> {
    let entry = CredentialEntry::deserialize(value)
        .map_err(|error| WireError::protocol(format!("not a wire credential: {error}")))?;
    Ok(WireCredential {
        reference: CredentialReference(entry.reference),
        kind: match entry.kind {
            KindEntry::ApiKey => CredentialKind::ApiKey,
            KindEntry::Oauth => CredentialKind::Oauth,
        },
    })
}
