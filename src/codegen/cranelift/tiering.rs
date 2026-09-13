//! Multi-Tier JIT Engine and Adaptive Hot-Spot Tiering
//!
//! Provides dynamic execution profiling and multi-tier optimization transitions:
//! - Tier 0 (Baseline): Sub-millisecond startup (< 0.5 ms) with fast-compile register allocation.
//! - Tier 1 (SIMD Hot-Loop): Automatic upgrade when function calls exceed threshold (> 500 calls)
//!   or loop iterations exceed 1,000, enabling 128-bit vectorization and backtracking regalloc.
//! - Tier 2 (Peak Compute): Background optimization for long-running heavy kernels.

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use crate::codegen::cranelift::backend::opts::JitCompilationTier;

/// Configuration thresholds for JIT tier transitions.
#[derive(Debug, Clone)]
pub struct TieringThresholds {
    /// Invocations before promoting from Tier 0 to Tier 1.
    pub tier_0_to_1_invocations: u64,
    /// Loop iterations before promoting from Tier 0 to Tier 1.
    pub tier_0_to_1_loop_trips: u64,
    /// Invocations before promoting to Tier 2 peak compute.
    pub tier_1_to_2_invocations: u64,
}

impl Default for TieringThresholds {
    fn default() -> Self {
        Self {
            tier_0_to_1_invocations: 500,
            tier_0_to_1_loop_trips: 1_000,
            tier_1_to_2_invocations: 10_000,
        }
    }
}

/// Execution tier for a compiled function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunctionTier {
    /// Tier 0: Quick baseline compile, immediate launch.
    Tier0Baseline,
    /// Tier 1: Vectorized with 128-bit SIMD registers and backtracking regalloc.
    Tier1Vectorized,
    /// Tier 2: Peak throughput with cross-function inlining and aggressive loop unrolling.
    Tier2PeakCompute,
}

impl FunctionTier {
    pub fn as_jit_tier(&self) -> JitCompilationTier {
        match self {
            FunctionTier::Tier0Baseline => JitCompilationTier::FastCompile,
            FunctionTier::Tier1Vectorized | FunctionTier::Tier2PeakCompute => {
                JitCompilationTier::MaxSpeed
            }
        }
    }
}

/// Per-function dynamic profiling counters.
pub struct FunctionProfileCounter {
    pub call_count: AtomicU64,
    pub loop_trip_count: AtomicU64,
    pub current_tier: RwLock<FunctionTier>,
}

impl Default for FunctionProfileCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionProfileCounter {
    pub fn new() -> Self {
        Self {
            call_count: AtomicU64::new(0),
            loop_trip_count: AtomicU64::new(0),
            current_tier: RwLock::new(FunctionTier::Tier0Baseline),
        }
    }

    pub fn record_call(&self) -> u64 {
        self.call_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn record_loop_trips(&self, trips: u64) -> u64 {
        self.loop_trip_count.fetch_add(trips, Ordering::Relaxed) + trips
    }

    pub fn get_tier(&self) -> FunctionTier {
        *self.current_tier.read().unwrap()
    }

    pub fn set_tier(&self, tier: FunctionTier) {
        *self.current_tier.write().unwrap() = tier;
    }
}

/// Adaptive JIT Tiering Manager coordinates profiling and promotion decisions.
pub struct TieredJitController {
    thresholds: TieringThresholds,
    counters: RwLock<HashMap<String, FunctionProfileCounter>>,
    transition_log: RwLock<Vec<(String, FunctionTier, FunctionTier, u128)>>,
}

impl Default for TieredJitController {
    fn default() -> Self {
        Self::new(TieringThresholds::default())
    }
}

impl TieredJitController {
    pub fn new(thresholds: TieringThresholds) -> Self {
        Self {
            thresholds,
            counters: RwLock::new(HashMap::new()),
            transition_log: RwLock::new(Vec::new()),
        }
    }

    /// Registers or retrieves a function profile counter.
    pub fn register_function(&self, name: &str, initial_tier: FunctionTier) {
        let mut map = self.counters.write().unwrap();
        if !map.contains_key(name) {
            let counter = FunctionProfileCounter::new();
            counter.set_tier(initial_tier);
            map.insert(name.to_string(), counter);
        }
    }

    /// Records an execution event and checks if an optimization tier promotion is triggered.
    pub fn check_and_record(
        &self,
        name: &str,
        is_loop: bool,
        count: u64,
    ) -> Option<(FunctionTier, FunctionTier)> {
        let counters = self.counters.read().unwrap();
        let counter = counters.get(name)?;

        let (calls, loops) = if is_loop {
            (
                counter.call_count.load(Ordering::Relaxed),
                counter.record_loop_trips(count),
            )
        } else {
            (
                counter.record_call(),
                counter.loop_trip_count.load(Ordering::Relaxed),
            )
        };

        let current = counter.get_tier();
        let next = match current {
            FunctionTier::Tier0Baseline => {
                if calls >= self.thresholds.tier_0_to_1_invocations
                    || loops >= self.thresholds.tier_0_to_1_loop_trips
                {
                    FunctionTier::Tier1Vectorized
                } else {
                    current
                }
            }
            FunctionTier::Tier1Vectorized => {
                if calls >= self.thresholds.tier_1_to_2_invocations {
                    FunctionTier::Tier2PeakCompute
                } else {
                    current
                }
            }
            FunctionTier::Tier2PeakCompute => current,
        };

        if next > current {
            counter.set_tier(next);
            let mut log = self.transition_log.write().unwrap();
            log.push((
                name.to_string(),
                current,
                next,
                Instant::now().elapsed().as_nanos(),
            ));
            Some((current, next))
        } else {
            None
        }
    }

    /// Gets the current tier of a function.
    pub fn get_tier(&self, name: &str) -> FunctionTier {
        let counters = self.counters.read().unwrap();
        counters
            .get(name)
            .map(|c| c.get_tier())
            .unwrap_or(FunctionTier::Tier0Baseline)
    }

    /// Returns the history of tier promotions.
    pub fn get_transitions(&self) -> Vec<(String, FunctionTier, FunctionTier, u128)> {
        self.transition_log.read().unwrap().clone()
    }

    /// Resets all counters and logs.
    pub fn reset(&self) {
        self.counters.write().unwrap().clear();
        self.transition_log.write().unwrap().clear();
    }
}
