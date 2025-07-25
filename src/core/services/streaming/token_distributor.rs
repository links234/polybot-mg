//! Token distribution logic for workers

use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

/// Distributes tokens across workers efficiently
#[derive(Debug)]
pub struct TokenDistributor {
    /// Maximum tokens per worker
    tokens_per_worker: usize,

    /// Current token assignments: worker_id -> token_set
    worker_assignments: HashMap<usize, HashSet<String>>,

    /// Reverse mapping: token -> worker_id
    token_to_worker: HashMap<String, usize>,

    /// Next worker ID to assign
    next_worker_id: usize,
}

impl TokenDistributor {
    pub fn new(tokens_per_worker: usize) -> Self {
        Self {
            tokens_per_worker,
            worker_assignments: HashMap::new(),
            token_to_worker: HashMap::new(),
            next_worker_id: 0,
        }
    }

    /// Add tokens and return the distribution changes
    pub fn add_tokens(&mut self, tokens: Vec<String>) -> DistributionUpdate {
        let mut update = DistributionUpdate::new();

        for token in tokens {
            if self.token_to_worker.contains_key(&token) {
                debug!("Token {} already assigned, skipping", token);
                continue;
            }

            // Find worker with least tokens or create new one
            let worker_id = self.find_or_create_worker_for_token();

            // Assign token to worker
            self.worker_assignments
                .entry(worker_id)
                .or_insert_with(HashSet::new)
                .insert(token.clone());

            self.token_to_worker.insert(token.clone(), worker_id);

            // Record the change
            update.add_token_to_worker(worker_id, token);

            info!("Assigned token to worker {}", worker_id);
        }

        update
    }


    /// Get worker ID for a specific token
    pub fn get_worker_for_token(&self, token: &str) -> Option<usize> {
        self.token_to_worker.get(token).cloned()
    }

    /// Get list of active worker IDs
    #[allow(dead_code)]
    pub fn get_active_workers(&self) -> Vec<usize> {
        self.worker_assignments.keys().cloned().collect()
    }

    /// Get total number of tokens being tracked
    #[allow(dead_code)]
    pub fn total_tokens(&self) -> usize {
        self.token_to_worker.len()
    }

    /// Get summary of distribution
    #[allow(dead_code)]
    pub fn get_summary(&self) -> DistributionSummary {
        let workers: Vec<WorkerInfo> = self.worker_assignments
            .iter()
            .map(|(worker_id, tokens)| WorkerInfo {
                worker_id: *worker_id,
                token_count: tokens.len(),
                tokens: tokens.iter().cloned().collect(),
            })
            .collect();

        DistributionSummary {
            total_workers: self.worker_assignments.len(),
            total_tokens: self.token_to_worker.len(),
            max_tokens_per_worker: self.tokens_per_worker,
            workers,
        }
    }

    /// Remove tokens and return distribution changes
    #[allow(dead_code)]
    pub fn remove_tokens(&mut self, tokens: Vec<String>) -> DistributionUpdate {
        let mut update = DistributionUpdate::new();

        for token in tokens {
            if let Some(worker_id) = self.token_to_worker.remove(&token) {
                // Remove token from worker assignment
                if let Some(worker_tokens) = self.worker_assignments.get_mut(&worker_id) {
                    worker_tokens.remove(&token);
                    
                    // Add to remove list
                    update.workers_to_remove
                        .entry(worker_id)
                        .or_insert_with(Vec::new)
                        .push(token);

                    // If worker has no tokens left, mark for shutdown
                    if worker_tokens.is_empty() {
                        self.worker_assignments.remove(&worker_id);
                        update.workers_to_shutdown.push(worker_id);
                    }
                }
            }
        }

        update
    }

    /// Find the best worker for a new token or create a new one
    fn find_or_create_worker_for_token(&mut self) -> usize {
        // Find worker with minimum tokens that's not at capacity
        let best_worker = self
            .worker_assignments
            .iter()
            .filter(|(_, tokens)| tokens.len() < self.tokens_per_worker)
            .min_by_key(|(_, tokens)| tokens.len())
            .map(|(worker_id, _)| *worker_id);

        match best_worker {
            Some(worker_id) => worker_id,
            None => {
                // All workers are at capacity, create new one
                let new_worker_id = self.next_worker_id;
                self.next_worker_id += 1;

                info!(
                    "Creating new worker {} for additional tokens",
                    new_worker_id
                );
                new_worker_id
            }
        }
    }
}

/// Summary of token distribution across workers
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DistributionSummary {
    pub total_workers: usize,
    pub total_tokens: usize,
    pub max_tokens_per_worker: usize,
    pub workers: Vec<WorkerInfo>,
}

/// Information about a specific worker
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WorkerInfo {
    pub worker_id: usize,
    pub token_count: usize,
    pub tokens: Vec<String>,
}

/// Represents changes to the token distribution
#[derive(Debug, Default)]
pub struct DistributionUpdate {
    /// Workers that need new tokens added: worker_id -> tokens
    pub workers_to_add: HashMap<usize, Vec<String>>,

    /// Workers that need tokens removed: worker_id -> tokens
    pub workers_to_remove: HashMap<usize, Vec<String>>,

    /// Workers that should be completely removed
    pub workers_to_shutdown: Vec<usize>,
}

impl DistributionUpdate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_token_to_worker(&mut self, worker_id: usize, token: String) {
        self.workers_to_add
            .entry(worker_id)
            .or_insert_with(Vec::new)
            .push(token);
    }


    /// Check if this update has any changes
    pub fn has_changes(&self) -> bool {
        !self.workers_to_add.is_empty()
            || !self.workers_to_remove.is_empty()
            || !self.workers_to_shutdown.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_distribution() {
        let mut distributor = TokenDistributor::new(3); // 3 tokens per worker for testing

        // Add 5 tokens - should create 2 workers
        let tokens = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
            "E".to_string(),
        ];
        let update = distributor.add_tokens(tokens);

        assert!(update.has_changes());
        assert_eq!(distributor.get_active_workers().len(), 2);
        assert_eq!(distributor.total_tokens(), 5);

        // Check distribution
        let summary = distributor.get_summary();
        assert_eq!(summary.total_workers, 2);
        assert_eq!(summary.total_tokens, 5);
        assert!(summary.max_tokens_per_worker <= 3);
    }

    #[test]
    fn test_token_removal() {
        let mut distributor = TokenDistributor::new(2);

        // Add tokens
        distributor.add_tokens(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert_eq!(distributor.get_active_workers().len(), 2);

        // Remove all tokens from one worker
        let update = distributor.remove_tokens(vec!["A".to_string(), "B".to_string()]);

        // Should clean up the empty worker
        assert!(
            update.workers_to_shutdown.len() > 0 || distributor.get_active_workers().len() == 1
        );
    }
}
