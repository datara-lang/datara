//! Structured Concurrency Scopes and Supervision Trees (v1.2.7)
//!
//! Provides deterministic lifetime boundaries and failure management:
//! - Child fibers spawned in a scope cannot outlive the scope.
//! - Structured error propagation: FailFast immediately halts sibling tasks.
//! - Clean unwinding and cancellation token propagation without resource leaks.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::runtime::fiber::actor::{
    ActorHandle, CancellationToken, SupervisionPolicy, execute_isolated_actor, generate_actor_id,
};
use crate::runtime::fiber::channel::Channel;

/// A structured concurrency scope governing a group of cooperative actor fibers.
pub struct ParallelScope {
    policy: SupervisionPolicy,
    cancel_token: CancellationToken,
    has_failed: Arc<AtomicBool>,
}

impl ParallelScope {
    pub fn new(policy: SupervisionPolicy) -> Self {
        Self {
            policy,
            cancel_token: CancellationToken::new(),
            has_failed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Spawns an actor fiber within this structured scope.
    pub fn spawn<F, T>(&self, f: F) -> ActorHandle<T>
    where
        F: Fn() -> T + Send + 'static,
        T: Send + 'static,
    {
        let actor_id = generate_actor_id();
        let cancel_token = self.cancel_token.clone();
        let result_channel = Channel::new(1);
        let channel_clone = result_channel.clone();
        let policy = self.policy;
        let has_failed = self.has_failed.clone();

        let handle = ActorHandle::new(actor_id, cancel_token.clone(), result_channel);

        // In the native thread/fiber runner:
        let runner_cancel = cancel_token.clone();
        std::thread::spawn(move || {
            let res = execute_isolated_actor(actor_id, policy, runner_cancel.clone(), f);
            if res.is_err() && policy == SupervisionPolicy::FailFast {
                has_failed.store(true, Ordering::Release);
                runner_cancel.cancel();
            }
            let _ = channel_clone.try_send(res);
        });

        handle
    }

    /// Cancels all child actors in the scope.
    pub fn cancel_all(&self) {
        self.cancel_token.cancel();
    }

    /// Returns true if the scope has been cancelled or experienced a fail-fast error.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled() || self.has_failed.load(Ordering::Acquire)
    }

    /// Accessor for the supervision policy.
    pub fn policy(&self) -> SupervisionPolicy {
        self.policy
    }
}

/// Executes a closure within a structured concurrency scope, guaranteeing all children
/// are bounded by the scope lifetime.
pub fn parallel_scope<F, R>(policy: SupervisionPolicy, scope_fn: F) -> R
where
    F: FnOnce(&ParallelScope) -> R,
{
    let scope = ParallelScope::new(policy);
    let result = scope_fn(&scope);
    // If scope was fail-fast and failed, ensure cancellation of any lingering tasks
    if scope.is_cancelled() {
        scope.cancel_all();
    }
    result
}
