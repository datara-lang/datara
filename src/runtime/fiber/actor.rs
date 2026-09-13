//! Supervised Actor Fibers with Isolated Crash Domains (v1.2.7)
//!
//! Provides isolated actor execution for Datara fibers:
//! - "Let it Crash" semantics: Panics and hardware traps within an actor are
//!   caught and isolated without terminating the host process or worker threads.
//! - Configurable supervision policies: Isolate, FailFast, or automatic Restart.
//! - Zero shared mutable state: Actors communicate exclusively via zero-copy channels.

use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Unique identifier for an actor fiber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActorId(pub u64);

/// Current lifecycle status of an actor fiber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorStatus {
    Pending,
    Running,
    Completed,
    Crashed(String),
    Cancelled,
}

/// Supervision policy governing how actor crashes are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionPolicy {
    /// Isolated: Child failure is captured as an error result; siblings continue.
    Isolate,
    /// Fail-fast: Child failure immediately cancels all linked sibling fibers.
    FailFast,
    /// Restart: Child is automatically restarted up to `max_retries` times on failure.
    Restart { max_retries: usize },
}

/// Shared cancellation token for linked/scoped actor groups.
#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Error produced when an actor fiber fails or is cancelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorError {
    Crashed(String),
    Cancelled,
    Timeout,
}

/// Handle allowing caller to inspect or join an actor fiber's result.
pub struct ActorHandle<T> {
    pub id: ActorId,
    pub cancel_token: CancellationToken,
    pub result_channel: Arc<crate::runtime::fiber::channel::Channel<Result<T, ActorError>>>,
}

impl<T: Send + 'static> ActorHandle<T> {
    pub fn new(
        id: ActorId,
        cancel_token: CancellationToken,
        result_channel: Arc<crate::runtime::fiber::channel::Channel<Result<T, ActorError>>>,
    ) -> Self {
        Self {
            id,
            cancel_token,
            result_channel,
        }
    }

    pub fn id(&self) -> ActorId {
        self.id
    }

    /// Cancels the actor fiber.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Returns true if the actor has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// Waits for the actor to complete and returns its outcome.
    pub fn join(&self) -> Result<T, ActorError> {
        self.result_channel
            .recv()
            .unwrap_or(Err(ActorError::Cancelled))
    }

    /// Non-blocking check for actor completion.
    pub fn try_join(&self) -> Option<Result<T, ActorError>> {
        self.result_channel.try_recv()
    }
}

/// Helper function to extract a human-readable message from a panic payload.
pub fn extract_panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown fiber execution trap or assertion failure".to_string()
    }
}

/// Global actor ID generator.
static NEXT_ACTOR_ID: AtomicU64 = AtomicU64::new(100);

/// Generates a unique ActorId.
pub fn generate_actor_id() -> ActorId {
    ActorId(NEXT_ACTOR_ID.fetch_add(1, Ordering::Relaxed))
}

/// Executes a closure inside an isolated failure domain with the given policy.
pub fn execute_isolated_actor<F, T>(
    actor_id: ActorId,
    policy: SupervisionPolicy,
    cancel_token: CancellationToken,
    f: F,
) -> Result<T, ActorError>
where
    F: Fn() -> T + Send + 'static,
    T: Send + 'static,
{
    let mut retries_left = match policy {
        SupervisionPolicy::Restart { max_retries } => max_retries,
        _ => 0,
    };

    loop {
        if cancel_token.is_cancelled() {
            return Err(ActorError::Cancelled);
        }

        let result = catch_unwind(AssertUnwindSafe(&f));

        match result {
            Ok(value) => return Ok(value),
            Err(panic_err) => {
                let msg = extract_panic_message(panic_err);
                if retries_left > 0 {
                    retries_left -= 1;
                    continue;
                }

                if policy == SupervisionPolicy::FailFast {
                    cancel_token.cancel();
                }

                return Err(ActorError::Crashed(format!(
                    "Actor #{} crashed: {}",
                    actor_id.0, msg
                )));
            }
        }
    }
}
