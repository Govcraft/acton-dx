// ! Job scheduling types and utilities.

use chrono::{DateTime, Duration, Utc};
use cron::Schedule as CronSchedule;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::JobError;

/// Job execution schedule.
///
/// Supports three types of scheduling:
/// - **Cron**: Execute on a cron schedule (e.g., "0 0 0 * * *" for daily at midnight)
/// - **Delayed**: Execute once after a delay
/// - **Recurring**: Execute repeatedly with a fixed interval
///
/// # Examples
///
/// ```rust
/// use acton_htmx::jobs::JobSchedule;
/// use std::time::Duration;
///
/// // Cron schedule: daily at midnight
/// let daily = JobSchedule::cron("0 0 0 * * *").unwrap();
///
/// // Delayed: run once after 1 hour
/// let delayed = JobSchedule::after(Duration::from_secs(3600));
///
/// // Recurring: run every minute
/// let recurring = JobSchedule::every(Duration::from_secs(60));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobSchedule {
    /// Execute on a cron schedule.
    ///
    /// Uses standard cron syntax with 6 fields:
    /// - `sec min hour day_of_month month day_of_week`
    ///
    /// Examples:
    /// - `"0 0 0 * * *"` - Daily at midnight
    /// - `"0 0 */2 * * *"` - Every 2 hours
    /// - `"0 0 9 * * 1-5"` - Weekdays at 9 AM
    /// - `"0 */15 * * * *"` - Every 15 minutes
    Cron {
        /// Cron expression string.
        expression: String,
        /// Parsed cron schedule (not serialized, boxed to reduce enum size).
        #[serde(skip)]
        schedule: Option<Box<CronSchedule>>,
    },

    /// Execute once after a delay from now.
    ///
    /// The job will be enqueued once the delay has elapsed.
    Delayed {
        /// Delay duration in milliseconds.
        #[serde(with = "duration_ms")]
        delay: std::time::Duration,
    },

    /// Execute repeatedly with a fixed interval.
    ///
    /// The job will be re-enqueued after each execution.
    Recurring {
        /// Interval duration in milliseconds.
        #[serde(with = "duration_ms")]
        interval: std::time::Duration,
        /// Optional maximum number of executions (None = infinite).
        max_executions: Option<u64>,
    },
}

impl JobSchedule {
    /// Create a cron-based schedule.
    ///
    /// # Errors
    ///
    /// Returns an error if the cron expression is invalid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::jobs::JobSchedule;
    ///
    /// // Daily at midnight
    /// let schedule = JobSchedule::cron("0 0 * * *").unwrap();
    ///
    /// // Every 15 minutes
    /// let schedule = JobSchedule::cron("*/15 * * * *").unwrap();
    /// ```
    pub fn cron(expression: &str) -> Result<Self, JobError> {
        let schedule = CronSchedule::from_str(expression)
            .map_err(|e| JobError::Other(format!("Invalid cron expression: {e}")))?;

        Ok(Self::Cron {
            expression: expression.to_string(),
            schedule: Some(Box::new(schedule)),
        })
    }

    /// Create a delayed schedule (execute once after delay).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::jobs::JobSchedule;
    /// use std::time::Duration;
    ///
    /// // Execute once after 1 hour
    /// let schedule = JobSchedule::after(Duration::from_secs(3600));
    /// ```
    #[must_use]
    pub const fn after(delay: std::time::Duration) -> Self {
        Self::Delayed { delay }
    }

    /// Create a recurring schedule (execute repeatedly).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::jobs::JobSchedule;
    /// use std::time::Duration;
    ///
    /// // Execute every minute, forever
    /// let schedule = JobSchedule::every(Duration::from_secs(60));
    ///
    /// // Execute every hour, max 10 times
    /// let schedule = JobSchedule::every(Duration::from_secs(3600))
    ///     .with_max_executions(10);
    /// ```
    #[must_use]
    pub const fn every(interval: std::time::Duration) -> Self {
        Self::Recurring {
            interval,
            max_executions: None,
        }
    }

    /// Set maximum number of executions for recurring schedules.
    ///
    /// Only applies to recurring schedules. Has no effect on cron or delayed schedules.
    #[must_use]
    pub fn with_max_executions(self, max: u64) -> Self {
        match self {
            Self::Recurring {
                interval,
                max_executions: _,
            } => Self::Recurring {
                interval,
                max_executions: Some(max),
            },
            other => other,
        }
    }

    /// Calculate the next execution time from the given reference time.
    ///
    /// # Returns
    ///
    /// - For cron schedules: Returns the next scheduled time according to the cron expression
    /// - For delayed schedules: Returns reference + delay
    /// - For recurring schedules: Returns reference + interval
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::jobs::JobSchedule;
    /// use chrono::Utc;
    /// use std::time::Duration;
    ///
    /// let schedule = JobSchedule::every(Duration::from_secs(60));
    /// let now = Utc::now();
    /// let next = schedule.next_execution(now).unwrap();
    ///
    /// assert!(next > now);
    /// ```
    #[must_use]
    pub fn next_execution(&self, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Cron {
                expression: _,
                schedule,
            } => {
                let sched = schedule.as_ref()?;
                sched.after(&from).next()
            }
            Self::Delayed { delay } => {
                let duration = Duration::from_std(*delay).ok()?;
                Some(from + duration)
            }
            Self::Recurring {
                interval,
                max_executions: _,
            } => {
                let duration = Duration::from_std(*interval).ok()?;
                Some(from + duration)
            }
        }
    }

    /// Check if the schedule has more executions remaining.
    ///
    /// For delayed schedules, this returns `false` after the first execution.
    /// For recurring schedules with `max_executions`, this checks the execution count.
    /// For cron schedules and infinite recurring, this always returns `true`.
    #[must_use]
    pub const fn has_more_executions(&self, executions: u64) -> bool {
        match self {
            Self::Cron { .. } => true,
            Self::Delayed { .. } => executions == 0,
            Self::Recurring {
                max_executions,
                interval: _,
            } => {
                if let Some(max) = max_executions {
                    executions < *max
                } else {
                    true
                }
            }
        }
    }

    /// Get a human-readable description of the schedule.
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::Cron { expression, .. } => format!("cron: {expression}"),
            Self::Delayed { delay } => {
                format!("delayed: {}s", delay.as_secs())
            }
            Self::Recurring {
                interval,
                max_executions,
            } => max_executions.as_ref().map_or_else(
                || format!("every {}s", interval.as_secs()),
                |max| format!("every {}s (max {max} times)", interval.as_secs()),
            ),
        }
    }
}

/// Serde helper for serializing Duration as milliseconds.
mod duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis().try_into().unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ms = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_schedule_parsing() {
        // Valid cron expressions (6 fields: sec min hour day month dayofweek)
        assert!(JobSchedule::cron("0 0 0 * * *").is_ok());  // Daily at midnight
        assert!(JobSchedule::cron("0 */15 * * * *").is_ok());  // Every 15 minutes
        assert!(JobSchedule::cron("0 0 9 * * 1-5").is_ok());  // Weekdays at 9 AM

        // Invalid cron expression
        assert!(JobSchedule::cron("invalid").is_err());
    }

    #[test]
    fn test_delayed_schedule() {
        let delay = std::time::Duration::from_secs(3600);
        let schedule = JobSchedule::after(delay);

        let now = Utc::now();
        let next = schedule.next_execution(now).unwrap();

        // Should be roughly 1 hour in the future (allow 1 second tolerance)
        let diff = next.signed_duration_since(now);
        assert!((diff.num_seconds() - 3600).abs() < 1);
    }

    #[test]
    fn test_recurring_schedule() {
        let interval = std::time::Duration::from_secs(60);
        let schedule = JobSchedule::every(interval);

        let now = Utc::now();
        let next = schedule.next_execution(now).unwrap();

        // Should be roughly 1 minute in the future
        let diff = next.signed_duration_since(now);
        assert!((diff.num_seconds() - 60).abs() < 1);
    }

    #[test]
    fn test_recurring_with_max_executions() {
        let schedule = JobSchedule::every(std::time::Duration::from_secs(60)).with_max_executions(5);

        assert!(schedule.has_more_executions(0));
        assert!(schedule.has_more_executions(4));
        assert!(!schedule.has_more_executions(5));
        assert!(!schedule.has_more_executions(10));
    }

    #[test]
    fn test_delayed_single_execution() {
        let schedule = JobSchedule::after(std::time::Duration::from_secs(60));

        assert!(schedule.has_more_executions(0));
        assert!(!schedule.has_more_executions(1));
    }

    #[test]
    fn test_cron_next_execution() {
        // Daily at midnight (6 fields)
        let schedule = JobSchedule::cron("0 0 0 * * *").unwrap();

        let now = Utc::now();
        let next = schedule.next_execution(now).unwrap();

        // Next execution should be in the future
        assert!(next > now);
    }

    #[test]
    fn test_schedule_description() {
        let cron = JobSchedule::cron("0 0 0 * * *").unwrap();
        assert!(cron.description().contains("cron"));

        let delayed = JobSchedule::after(std::time::Duration::from_secs(3600));
        assert!(delayed.description().contains("3600"));

        let recurring = JobSchedule::every(std::time::Duration::from_secs(60));
        assert!(recurring.description().contains("60"));
    }

    #[test]
    fn test_serialization() {
        let schedule = JobSchedule::every(std::time::Duration::from_secs(60)).with_max_executions(5);

        let json = serde_json::to_string(&schedule).unwrap();
        let deserialized: JobSchedule = serde_json::from_str(&json).unwrap();

        // Verify it round-trips correctly
        match deserialized {
            JobSchedule::Recurring {
                interval,
                max_executions,
            } => {
                assert_eq!(interval.as_secs(), 60);
                assert_eq!(max_executions, Some(5));
            }
            _ => panic!("Expected Recurring schedule"),
        }
    }
}
