//! Data service client for database operations.

use super::error::ClientError;
use acton_dx_proto::data::v1::{
    data_service_client::DataServiceClient, BeginTransactionRequest, CommitTransactionRequest,
    ExecuteRequest, MigrationInfo, MigrationStatusRequest, PingRequest, QueryRequest,
    RollbackTransactionRequest, Row, RunMigrationsRequest, TransactionExecuteRequest, Value,
};
use tonic::transport::Channel;

/// Client for the data service.
///
/// Provides database query execution, transactions, and migration management.
#[derive(Debug, Clone)]
pub struct DataClient {
    client: DataServiceClient<Channel>,
}

impl DataClient {
    /// Connect to the data service.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, ClientError> {
        let endpoint = endpoint.into();
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| ClientError::ConnectionFailed(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            client: DataServiceClient::new(channel),
        })
    }

    // ==================== Query Operations ====================

    /// Execute a query and return multiple rows.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn query(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        transaction_id: Option<String>,
    ) -> Result<Vec<Row>, ClientError> {
        let response = self
            .client
            .query(QueryRequest {
                sql: sql.to_string(),
                params,
                transaction_id,
            })
            .await?;

        Ok(response.into_inner().rows)
    }

    /// Execute a query and return a single row.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn query_one(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        transaction_id: Option<String>,
    ) -> Result<Option<Row>, ClientError> {
        let response = self
            .client
            .query_one(QueryRequest {
                sql: sql.to_string(),
                params,
                transaction_id,
            })
            .await?;

        Ok(response.into_inner().row)
    }

    /// Execute a statement (INSERT, UPDATE, DELETE).
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        transaction_id: Option<String>,
    ) -> Result<ExecuteResult, ClientError> {
        let response = self
            .client
            .execute(ExecuteRequest {
                sql: sql.to_string(),
                params,
                transaction_id,
            })
            .await?;

        let inner = response.into_inner();
        Ok(ExecuteResult {
            rows_affected: inner.rows_affected,
            last_insert_id: inner.last_insert_id,
        })
    }

    // ==================== Transaction Operations ====================

    /// Begin a new transaction.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn begin_transaction(&mut self) -> Result<String, ClientError> {
        let response = self
            .client
            .begin_transaction(BeginTransactionRequest {})
            .await?;

        Ok(response.into_inner().transaction_id)
    }

    /// Commit a transaction.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn commit_transaction(&mut self, transaction_id: &str) -> Result<bool, ClientError> {
        let response = self
            .client
            .commit_transaction(CommitTransactionRequest {
                transaction_id: transaction_id.to_string(),
            })
            .await?;

        Ok(response.into_inner().success)
    }

    /// Rollback a transaction.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn rollback_transaction(
        &mut self,
        transaction_id: &str,
    ) -> Result<bool, ClientError> {
        let response = self
            .client
            .rollback_transaction(RollbackTransactionRequest {
                transaction_id: transaction_id.to_string(),
            })
            .await?;

        Ok(response.into_inner().success)
    }

    /// Execute a statement within a transaction.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn execute_in_transaction(
        &mut self,
        transaction_id: &str,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<ExecuteResult, ClientError> {
        let response = self
            .client
            .execute_in_transaction(TransactionExecuteRequest {
                transaction_id: transaction_id.to_string(),
                sql: sql.to_string(),
                params,
            })
            .await?;

        let inner = response.into_inner();
        Ok(ExecuteResult {
            rows_affected: inner.rows_affected,
            last_insert_id: inner.last_insert_id,
        })
    }

    // ==================== Migration Operations ====================

    /// Run database migrations.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn run_migrations(
        &mut self,
        migrations_path: &str,
    ) -> Result<MigrationResult, ClientError> {
        let response = self
            .client
            .run_migrations(RunMigrationsRequest {
                migrations_path: migrations_path.to_string(),
            })
            .await?;

        let inner = response.into_inner();
        Ok(MigrationResult {
            success: inner.success,
            migrations_run: inner.migrations_run,
            message: inner.message,
        })
    }

    /// Get migration status.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn migration_status(&mut self) -> Result<Vec<MigrationInfo>, ClientError> {
        let response = self
            .client
            .migration_status(MigrationStatusRequest {})
            .await?;

        Ok(response.into_inner().migrations)
    }

    // ==================== Health Operations ====================

    /// Ping the database to check health.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn ping(&mut self) -> Result<PingResult, ClientError> {
        let response = self.client.ping(PingRequest {}).await?;

        let inner = response.into_inner();
        Ok(PingResult {
            healthy: inner.healthy,
            latency_ms: inner.latency_ms,
        })
    }
}

/// Result of an execute operation.
#[derive(Debug, Clone)]
pub struct ExecuteResult {
    /// Number of rows affected.
    pub rows_affected: i64,
    /// Last insert ID if applicable.
    pub last_insert_id: Option<i64>,
}

/// Result of a migration operation.
#[derive(Debug, Clone)]
pub struct MigrationResult {
    /// Whether the migration succeeded.
    pub success: bool,
    /// Number of migrations run.
    pub migrations_run: i32,
    /// Status message.
    pub message: String,
}

/// Result of a ping operation.
#[derive(Debug, Clone)]
pub struct PingResult {
    /// Whether the database is healthy.
    pub healthy: bool,
    /// Latency in milliseconds.
    pub latency_ms: i64,
}
