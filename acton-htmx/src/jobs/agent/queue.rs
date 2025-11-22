//! Priority queue for jobs.

use crate::jobs::JobId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::time::Duration;

/// A job in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct QueuedJob {
    /// Unique job identifier.
    pub id: JobId,
    /// Job type name.
    pub job_type: String,
    /// Serialized job payload.
    pub payload: Vec<u8>,
    /// Job priority (higher = more important).
    pub priority: i32,
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Job execution timeout.
    pub timeout: Duration,
    /// When the job was enqueued.
    pub enqueued_at: DateTime<Utc>,
    /// Current attempt number (0 = first attempt).
    pub attempt: u32,
}

/// Wrapper for priority queue ordering.
#[derive(Debug, Clone)]
struct QueueEntry {
    job: QueuedJob,
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.job.priority == other.job.priority
            && self.job.enqueued_at == other.job.enqueued_at
    }
}

impl Eq for QueueEntry {}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first
        match other.job.priority.cmp(&self.job.priority) {
            Ordering::Equal => {
                // If same priority, older jobs first (FIFO)
                self.job.enqueued_at.cmp(&other.job.enqueued_at)
            }
            ord => ord,
        }
    }
}

/// Priority-based job queue.
#[derive(Debug)]
pub(crate) struct JobQueue {
    /// Binary heap for priority ordering.
    heap: BinaryHeap<QueueEntry>,
    /// Set of job IDs for O(1) contains check.
    ids: HashSet<JobId>,
    /// Maximum queue size.
    max_size: usize,
}

impl JobQueue {
    /// Create a new job queue with maximum size.
    #[must_use]
    pub(super) fn new(max_size: usize) -> Self {
        Self {
            heap: BinaryHeap::new(),
            ids: HashSet::new(),
            max_size,
        }
    }

    /// Enqueue a job.
    ///
    /// # Errors
    ///
    /// Returns an error if the queue is full or the job is already queued.
    pub(super) fn enqueue(&mut self, job: QueuedJob) -> Result<(), String> {
        if self.heap.len() >= self.max_size {
            return Err(format!("Queue is full (max: {})", self.max_size));
        }

        if self.ids.contains(&job.id) {
            return Err(format!("Job {} is already queued", job.id));
        }

        self.ids.insert(job.id);
        self.heap.push(QueueEntry { job });
        Ok(())
    }

    /// Check if a job is in the queue.
    #[must_use]
    pub(super) fn contains(&self, id: &JobId) -> bool {
        self.ids.contains(id)
    }
}

