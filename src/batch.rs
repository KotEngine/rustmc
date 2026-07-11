//! Concurrent status ping of many servers (Java and/or Bedrock) at once.
//! Async-only (needs `futures::stream::buffer_unordered` for bounded
//! concurrency) — there's no sync equivalent, spawning OS threads per
//! server doesn't scale the same way and is left to the caller if wanted.
//!
//! No retries at the batch level, intentionally: `JavaServer`/
//! `BedrockServer` already retry internally (default 3x, 250ms delay), and
//! retrying *again* per-target inside a batch would multiply worst-case
//! latency for the whole batch for just one unreachable target. A caller
//! who wants a specific target retried harder can call it directly instead
//! of through `ping_many`.

use std::time::Duration;

use futures::stream::{self, Stream, StreamExt};

use crate::error::RustmcError;
use crate::response::{BedrockStatusResponse, JavaStatusResponse};
use crate::server::{BedrockServer, JavaServer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edition {
    Java,
    Bedrock,
}

/// Unified response type, preserving each edition's full response without
/// a lossy conversion between them (Bedrock's `gamemode`/`map_name` have
/// no Java equivalent and vice versa) — match on this to get at
/// edition-specific fields.
#[derive(Debug, Clone)]
pub enum ServerStatus {
    Java(JavaStatusResponse),
    Bedrock(BedrockStatusResponse),
}

#[derive(Clone)]
pub struct ServerTarget {
    pub address: String,
    pub edition: Edition,
    /// Per-target timeout. If `None`, falls back to the `default_timeout`
    /// passed into `ping_many`/`ping_many_stream`. Useful when the batch
    /// mixes local and far-away servers that need different timeout
    /// budgets.
    pub timeout: Option<Duration>,
}

impl ServerTarget {
    /// Uses whatever `default_timeout` is passed into `ping_many`.
    pub fn new(address: impl Into<String>, edition: Edition) -> Self {
        Self { address: address.into(), edition, timeout: None }
    }

    /// Overrides the timeout for this specific target.
    pub fn with_timeout(address: impl Into<String>, edition: Edition, timeout: Duration) -> Self {
        Self { address: address.into(), edition, timeout: Some(timeout) }
    }

    fn effective_timeout(&self, default_timeout: Duration) -> Duration {
        self.timeout.unwrap_or(default_timeout)
    }
}

async fn ping_one(
    target: ServerTarget,
    default_timeout: Duration,
) -> (ServerTarget, Result<ServerStatus, RustmcError>) {
    let timeout = target.effective_timeout(default_timeout);

    let result = match target.edition {
        Edition::Java => async {
            let server = JavaServer::lookup_async(&target.address).await?.with_timeout(timeout);
            server.status_async().await.map(ServerStatus::Java)
        }
        .await,
        Edition::Bedrock => async {
            let server = BedrockServer::lookup(&target.address)?.with_timeout(timeout);
            server.status_async().await.map(ServerStatus::Bedrock)
        }
        .await,
    };

    (target, result)
}

/// Requests status from every target in `targets`, `max_parallel` at a
/// time, and collects all results (success and failure) once every
/// request has finished. Order of results is not guaranteed to match
/// `targets` — whichever finishes first comes back first.
pub async fn ping_many(
    targets: &[ServerTarget],
    max_parallel: usize,
    default_timeout: Duration,
) -> Vec<(ServerTarget, Result<ServerStatus, RustmcError>)> {
    ping_many_stream(targets, max_parallel, default_timeout).collect().await
}

/// Same as `ping_many`, but yields results as they complete instead of
/// collecting them all first — useful for showing progress on a large
/// batch (hundreds/thousands of targets) rather than waiting for the
/// slowest/timed-out one before seeing anything.
pub fn ping_many_stream(
    targets: &[ServerTarget],
    max_parallel: usize,
    default_timeout: Duration,
) -> impl Stream<Item = (ServerTarget, Result<ServerStatus, RustmcError>)> + '_ {
    stream::iter(targets.iter().cloned())
        .map(move |target| ping_one(target, default_timeout))
        .buffer_unordered(max_parallel.max(1))
}
