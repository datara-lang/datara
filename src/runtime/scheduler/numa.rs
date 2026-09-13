//! NUMA Domain & Core Affinity Topology Manager (v1.2.5)
//!
//! Pinning worker threads to physical CPU cores prevents kernel scheduler
//! thrashing across NUMA nodes, maintaining maximum L1/L2/L3 cache residency.

/// CPU Topology and NUMA node descriptor.
#[derive(Debug, Clone)]
pub struct NumaTopology {
    pub logical_cores: usize,
    pub physical_cores: usize,
    pub numa_nodes: usize,
}

impl Default for NumaTopology {
    fn default() -> Self {
        Self::detect()
    }
}

impl NumaTopology {
    /// Detects the host machine's hardware topology.
    pub fn detect() -> Self {
        let logical = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        // Approximation: physical cores typically half of logical cores on hyperthreaded machines
        let physical = (logical / 2).max(1);
        let numa_nodes = if logical >= 32 { 2 } else { 1 };

        Self {
            logical_cores: logical,
            physical_cores: physical,
            numa_nodes,
        }
    }

    /// Calculates the optimal target core index for worker thread ID.
    pub fn target_core_for_worker(&self, worker_id: usize) -> usize {
        worker_id % self.logical_cores
    }

    /// Calculates the NUMA node index for worker thread ID.
    pub fn numa_node_for_worker(&self, worker_id: usize) -> usize {
        (worker_id / (self.logical_cores / self.numa_nodes)).min(self.numa_nodes - 1)
    }
}
