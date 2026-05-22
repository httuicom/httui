//! Cancel-aware execution plumbing for executable blocks (DB + HTTP).
//!
//! Provides three pieces:
//! 1. `ExecutionRegistry` — maps `execution_id` strings to a
//!    `CancellationToken` so a separate invocation can cancel an in-flight
//!    query/request by id.
//! 2. Tauri commands:
//!    - `execute_db_streamed(params, execution_id, on_chunk)` runs a DB
//!      query and emits its final `DbChunk` on a `tauri::Channel`.
//!    - `execute_http_streamed(params, execution_id, on_chunk)` mirrors the
//!      same pattern for HTTP requests, emitting `HttpChunk`.
//!    - `cancel_block(execution_id)` signals the stored token.
//!
//! The existing synchronous `execute_block` command stays intact; the
//! streamed paths are plumbing for the UI redesign work and don't change
//! current behavior.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use httui_core::executor::db::{types::DbChunk, DbExecutor};
use httui_core::executor::http::{types::HttpChunk, HttpExecutor};
use httui_core::executor::Executor;
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

/// Registry of in-flight cancellable executions keyed by `execution_id`.
/// Cloneable via internal `Arc<Mutex>`.
#[derive(Clone, Default)]
pub struct ExecutionRegistry {
    inner: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl ExecutionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a fresh token under `id`. Overwrites any existing entry (old token stays valid via move semantics).
    pub fn register(&self, id: impl Into<String>) -> CancellationToken {
        let token = CancellationToken::new();
        let mut map = self.inner.lock().expect("execution registry poisoned");
        map.insert(id.into(), token.clone());
        token
    }

    /// Remove an id from the registry.
    pub fn unregister(&self, id: &str) {
        let mut map = self.inner.lock().expect("execution registry poisoned");
        map.remove(id);
    }

    /// Signal cancellation for `id`. Returns `true` if the id was present.
    pub fn cancel(&self, id: &str) -> bool {
        let map = self.inner.lock().expect("execution registry poisoned");
        if let Some(token) = map.get(id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("execution registry poisoned")
            .len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.inner
            .lock()
            .expect("execution registry poisoned")
            .is_empty()
    }
}

/// Run a DB query and emit its terminal chunk on the channel.
/// Errors are delivered in-band (`DbChunk::Error` / `DbChunk::Cancelled`); the command always returns `Ok(())`.
#[tauri::command]
pub async fn execute_db_streamed(
    db_executor: tauri::State<'_, Arc<DbExecutor>>,
    executions: tauri::State<'_, ExecutionRegistry>,
    params: serde_json::Value,
    execution_id: String,
    on_chunk: Channel<DbChunk>,
) -> Result<(), String> {
    db_executor
        .validate(&params)
        .await
        .map_err(|e| e.to_string())?;

    let token = executions.register(&execution_id);
    let result = db_executor.execute_with_cancel(params, token.clone()).await;
    executions.unregister(&execution_id);

    let chunk = match result {
        Ok(response) => DbChunk::Complete(response),
        Err(e) => {
            let msg = e.to_string();
            if msg == "Query cancelled" {
                DbChunk::Cancelled
            } else {
                DbChunk::Error { message: msg }
            }
        }
    };

    let _ = on_chunk.send(chunk);

    Ok(())
}

/// Run an HTTP request and stream its chunks on the channel.
///
/// Wire order: `Headers` → `BodyChunk`× N → `Complete`. On cancel emits `HttpChunk::Cancelled`;
/// transport failures emit `HttpChunk::Error`. The executor emits `Complete` on success —
/// this command only emits a terminal chunk on the error/cancel paths.
#[tauri::command]
pub async fn execute_http_streamed(
    http_executor: tauri::State<'_, Arc<HttpExecutor>>,
    executions: tauri::State<'_, ExecutionRegistry>,
    params: serde_json::Value,
    execution_id: String,
    on_chunk: Channel<HttpChunk>,
) -> Result<(), String> {
    http_executor
        .validate(&params)
        .await
        .map_err(|e| e.to_string())?;

    let token = executions.register(&execution_id);
    let on_chunk_for_executor = on_chunk.clone();
    let result = http_executor
        .execute_streamed(params, token.clone(), move |chunk| {
            let _ = on_chunk_for_executor.send(chunk);
        })
        .await;
    executions.unregister(&execution_id);

    if let Err(e) = result {
        let msg = e.to_string();
        let chunk = if msg == "Request cancelled" {
            HttpChunk::Cancelled
        } else {
            HttpChunk::Error { message: msg }
        };
        let _ = on_chunk.send(chunk);
    }
    Ok(())
}

/// Signal cancellation for an in-flight execution. No-op if the id is
/// unknown (the execution may have already finished).
#[tauri::command]
pub async fn cancel_block(
    executions: tauri::State<'_, ExecutionRegistry>,
    execution_id: String,
) -> Result<bool, String> {
    Ok(executions.cancel(&execution_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn registry_register_and_cancel() {
        let registry = ExecutionRegistry::new();
        let token = registry.register("abc");
        assert_eq!(registry.len(), 1);
        assert!(!token.is_cancelled());

        assert!(registry.cancel("abc"));
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn registry_cancel_unknown_returns_false() {
        let registry = ExecutionRegistry::new();
        assert!(!registry.cancel("unknown"));
    }

    #[tokio::test]
    async fn registry_unregister_removes_entry() {
        let registry = ExecutionRegistry::new();
        let _t = registry.register("abc");
        assert_eq!(registry.len(), 1);
        registry.unregister("abc");
        assert_eq!(registry.len(), 0);
    }

    #[tokio::test]
    async fn registry_is_cloneable_and_shares_state() {
        let a = ExecutionRegistry::new();
        let b = a.clone();
        let token = a.register("x");
        assert!(b.cancel("x"));
        assert!(token.is_cancelled());
    }
}
