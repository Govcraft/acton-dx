//! Job history tracking with bounded circular buffer.

use crate::jobs::JobId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Simplified job status for history tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryStatus {
    /// Job completed successfully.
    Completed,
    /// Job failed after all retry attempts.
    Failed,
}

/// A completed job record in the history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHistoryRecord {
    /// Unique job identifier.
    pub id: JobId,
    /// Job type name.
    pub job_type: String,
    /// Final job status (Completed or Failed).
    pub status: HistoryStatus,
    /// When the job was enqueued.
    pub enqueued_at: DateTime<Utc>,
    /// When the job started executing.
    pub started_at: DateTime<Utc>,
    /// When the job finished (completed or failed).
    pub finished_at: DateTime<Utc>,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Number of attempts before completion/failure.
    pub attempts: u32,
    /// Error message if failed.
    pub error_message: Option<String>,
}

impl JobHistoryRecord {
    /// Create a new completed job history record.
    #[must_use]
    pub fn completed(
        id: JobId,
        job_type: String,
        enqueued_at: DateTime<Utc>,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
        attempts: u32,
    ) -> Self {
        let duration_ms = (finished_at - started_at)
            .num_milliseconds()
            .max(0)
            .try_into()
            .unwrap_or(0);

        Self {
            id,
            job_type,
            status: HistoryStatus::Completed,
            enqueued_at,
            started_at,
            finished_at,
            duration_ms,
            attempts,
            error_message: None,
        }
    }

    /// Create a new failed job history record.
    #[must_use]
    pub fn failed(
        id: JobId,
        job_type: String,
        enqueued_at: DateTime<Utc>,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
        attempts: u32,
        error_message: String,
    ) -> Self {
        let duration_ms = (finished_at - started_at)
            .num_milliseconds()
            .max(0)
            .try_into()
            .unwrap_or(0);

        Self {
            id,
            job_type,
            status: HistoryStatus::Failed,
            enqueued_at,
            started_at,
            finished_at,
            duration_ms,
            attempts,
            error_message: Some(error_message),
        }
    }

    /// Check if this record matches a search query.
    ///
    /// Searches in job_type, job_id, and error_message fields.
    #[must_use]
    pub fn matches_search(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }

        let query_lower = query.to_lowercase();

        // Search in job type
        if self.job_type.to_lowercase().contains(&query_lower) {
            return true;
        }

        // Search in job ID
        if self.id.to_string().to_lowercase().contains(&query_lower) {
            return true;
        }

        // Search in error message if present
        if let Some(error) = &self.error_message {
            if error.to_lowercase().contains(&query_lower) {
                return true;
            }
        }

        false
    }
}

/// Bounded circular buffer for job history.
///
/// Maintains a fixed-size history of completed jobs using a circular buffer.
/// When capacity is reached, the oldest records are automatically evicted.
#[derive(Debug)]
pub(super) struct JobHistory {
    /// Circular buffer of job records.
    records: VecDeque<JobHistoryRecord>,
    /// Maximum number of records to keep.
    #[allow(dead_code)] // Used for capacity management
    max_records: usize,
}

impl JobHistory {
    /// Create a new job history with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `max_records` - Maximum number of job records to retain
    #[must_use]
    pub(super) fn new(max_records: usize) -> Self {
        Self {
            records: VecDeque::with_capacity(max_records),
            max_records,
        }
    }

    /// Add a job record to the history.
    ///
    /// If at capacity, the oldest record is automatically evicted.
    #[allow(dead_code)] // Will be used when jobs complete
    pub(super) fn add(&mut self, record: JobHistoryRecord) {
        if self.records.len() >= self.max_records {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }

    /// Get paginated job history with optional search filter.
    ///
    /// # Arguments
    ///
    /// * `page` - Page number (1-indexed)
    /// * `page_size` - Number of records per page
    /// * `search_query` - Optional search string to filter records
    ///
    /// # Returns
    ///
    /// Tuple of (records, total_count) where records are the page results
    /// and total_count is the total number of matching records.
    #[must_use]
    pub(super) fn get_page(
        &self,
        page: usize,
        page_size: usize,
        search_query: Option<&str>,
    ) -> (Vec<JobHistoryRecord>, usize) {
        // Filter records by search query
        let filtered: Vec<_> = search_query.map_or_else(
            || self.records.iter().rev().cloned().collect(),
            |query| {
                self.records
                    .iter()
                    .rev() // Most recent first
                    .filter(|record| record.matches_search(query))
                    .cloned()
                    .collect()
            },
        );

        let total_count = filtered.len();

        // Calculate pagination
        let page = page.max(1); // Ensure page is at least 1
        let start_index = (page - 1) * page_size;

        if start_index >= total_count {
            return (Vec::new(), total_count);
        }

        let end_index = (start_index + page_size).min(total_count);
        let page_records = filtered[start_index..end_index].to_vec();

        (page_records, total_count)
    }

    /// Get the total number of records in history.
    #[must_use]
    pub(super) fn len(&self) -> usize {
        self.records.len()
    }

    /// Check if history is empty.
    #[must_use]
    #[allow(dead_code)] // Will be used for history management
    pub(super) fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_record(id_num: u128, job_type: &str, status: HistoryStatus) -> JobHistoryRecord {
        let now = Utc::now();
        let started = now - chrono::Duration::seconds(10);
        let enqueued = started - chrono::Duration::seconds(5);
        let id = JobId::from(Uuid::from_u128(id_num));

        match status {
            HistoryStatus::Completed => {
                JobHistoryRecord::completed(id, job_type.to_string(), enqueued, started, now, 1)
            }
            HistoryStatus::Failed => JobHistoryRecord::failed(
                id,
                job_type.to_string(),
                enqueued,
                started,
                now,
                3,
                "Test error".to_string(),
            ),
        }
    }

    #[test]
    fn test_history_bounded_capacity() {
        let mut history = JobHistory::new(3);

        // Add 5 records to a capacity-3 history
        for i in 1..=5 {
            let record = create_test_record(i, "TestJob", HistoryStatus::Completed);
            history.add(record);
        }

        // Should only have last 3 records
        assert_eq!(history.len(), 3);

        let (records, _) = history.get_page(1, 10, None);
        assert_eq!(records.len(), 3);

        // Should have records 3, 4, 5 (most recent first)
        assert_eq!(*records[0].id.as_uuid(), Uuid::from_u128(5));
        assert_eq!(*records[1].id.as_uuid(), Uuid::from_u128(4));
        assert_eq!(*records[2].id.as_uuid(), Uuid::from_u128(3));
    }

    #[test]
    fn test_history_pagination() {
        let mut history = JobHistory::new(100);

        // Add 25 records
        for i in 1..=25 {
            let record = create_test_record(i, "TestJob", HistoryStatus::Completed);
            history.add(record);
        }

        // Get first page (10 records)
        let (page1, total) = history.get_page(1, 10, None);
        assert_eq!(page1.len(), 10);
        assert_eq!(total, 25);
        assert_eq!(*page1[0].id.as_uuid(), Uuid::from_u128(25)); // Most recent first

        // Get second page
        let (page2, total) = history.get_page(2, 10, None);
        assert_eq!(page2.len(), 10);
        assert_eq!(total, 25);
        assert_eq!(*page2[0].id.as_uuid(), Uuid::from_u128(15));

        // Get third page (partial)
        let (page3, total) = history.get_page(3, 10, None);
        assert_eq!(page3.len(), 5);
        assert_eq!(total, 25);
        assert_eq!(*page3[0].id.as_uuid(), Uuid::from_u128(5));

        // Get out of bounds page
        let (page4, total) = history.get_page(4, 10, None);
        assert_eq!(page4.len(), 0);
        assert_eq!(total, 25);
    }

    #[test]
    fn test_history_search() {
        let mut history = JobHistory::new(100);

        // Add records with different job types
        history.add(create_test_record(1, "SendEmail", HistoryStatus::Completed));
        history.add(create_test_record(2, "ProcessImage", HistoryStatus::Completed));
        history.add(create_test_record(3, "SendEmail", HistoryStatus::Failed));
        history.add(create_test_record(4, "GenerateReport", HistoryStatus::Completed));

        // Search by job type
        let (results, total) = history.get_page(1, 10, Some("SendEmail"));
        assert_eq!(results.len(), 2);
        assert_eq!(total, 2);
        assert_eq!(*results[0].id.as_uuid(), Uuid::from_u128(3)); // Most recent first
        assert_eq!(*results[1].id.as_uuid(), Uuid::from_u128(1));

        // Search by partial match
        let (results, total) = history.get_page(1, 10, Some("email"));
        assert_eq!(results.len(), 2);
        assert_eq!(total, 2);

        // Search with no matches
        let (results, total) = history.get_page(1, 10, Some("NonExistent"));
        assert_eq!(results.len(), 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn test_record_matches_search() {
        let record = JobHistoryRecord::failed(
            JobId::from(Uuid::from_u128(123)),
            "SendEmail".to_string(),
            Utc::now(),
            Utc::now(),
            Utc::now(),
            2,
            "SMTP connection failed".to_string(),
        );

        // Should match job type
        assert!(record.matches_search("SendEmail"));
        assert!(record.matches_search("email"));
        assert!(record.matches_search("SEND"));

        // Should match error message
        assert!(record.matches_search("SMTP"));
        assert!(record.matches_search("connection"));

        // Should match job ID (UUID representation contains 7b which is hex for 123)
        assert!(record.matches_search("7b"));

        // Should not match
        assert!(!record.matches_search("xyz999"));

        // Empty query matches everything
        assert!(record.matches_search(""));
    }

    #[test]
    fn test_record_duration_calculation() {
        let enqueued = Utc::now();
        let started = enqueued + chrono::Duration::seconds(5);
        let finished = started + chrono::Duration::milliseconds(1500);

        let record = JobHistoryRecord::completed(
            JobId::from(Uuid::from_u128(1)),
            "TestJob".to_string(),
            enqueued,
            started,
            finished,
            1,
        );

        assert_eq!(record.duration_ms, 1500);
    }
}
