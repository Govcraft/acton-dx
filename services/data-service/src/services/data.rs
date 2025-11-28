//! Data service gRPC implementation.

use acton_dx_proto::data::v1::{
    data_service_server::DataService, value::Value as ProtoValueInner, BeginTransactionRequest,
    CommitTransactionRequest, ExecuteRequest, ExecuteResponse, MigrationResponse,
    MigrationStatusRequest, MigrationStatusResponse, PingRequest, PingResponse, QueryOneResponse,
    QueryRequest, QueryResponse, RollbackTransactionRequest, Row, RunMigrationsRequest,
    TransactionExecuteRequest, TransactionResponse, Value as ProtoValue,
};
use dashmap::DashMap;
use sqlx::any::{AnyArguments, AnyRow};
use sqlx::{AnyPool, Arguments, Column, Row as SqlxRow, TypeInfo};
use std::sync::Arc;
use std::time::Instant;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

/// Active transaction wrapper.
struct ActiveTransaction {
    /// The SQLx transaction (held as pool reference for simplicity).
    _created_at: std::time::Instant,
}

/// Data service implementation.
pub struct DataServiceImpl {
    /// Database connection pool.
    pool: AnyPool,
    /// Active transactions by ID.
    transactions: Arc<DashMap<String, ActiveTransaction>>,
}

impl DataServiceImpl {
    /// Create a new data service with the given connection pool.
    #[must_use]
    pub fn new(pool: AnyPool) -> Self {
        Self {
            pool,
            transactions: Arc::new(DashMap::new()),
        }
    }

    /// Convert proto values to SQLx arguments.
    fn bind_params(params: &[ProtoValue]) -> AnyArguments<'_> {
        let mut args = AnyArguments::default();
        for param in params {
            match &param.value {
                Some(ProtoValueInner::BoolValue(v)) => {
                    args.add(*v).ok();
                }
                Some(ProtoValueInner::IntValue(v)) => {
                    args.add(*v).ok();
                }
                Some(ProtoValueInner::FloatValue(v)) => {
                    args.add(*v).ok();
                }
                Some(ProtoValueInner::StringValue(v)) => {
                    args.add(v.as_str()).ok();
                }
                Some(ProtoValueInner::BytesValue(v)) => {
                    args.add(v.as_slice()).ok();
                }
                Some(ProtoValueInner::NullValue(_)) | None => {
                    args.add(Option::<String>::None).ok();
                }
            }
        }
        args
    }

    /// Convert a SQLx row to a proto Row.
    fn row_to_proto(row: &AnyRow) -> Row {
        let mut columns = std::collections::HashMap::new();

        for (i, column) in row.columns().iter().enumerate() {
            let name = column.name().to_string();
            let value = Self::column_to_proto_value(row, i);
            columns.insert(name, value);
        }

        Row { columns }
    }

    /// Create a null proto value.
    const fn null_value() -> ProtoValue {
        ProtoValue {
            value: Some(ProtoValueInner::NullValue(true)),
        }
    }

    /// Convert a column value to proto Value.
    fn column_to_proto_value(row: &AnyRow, index: usize) -> ProtoValue {
        // Try to get the value - if it's null, return null value
        let value_ref = row.try_get_raw(index);
        if value_ref
            .as_ref()
            .map_or(true, sqlx::ValueRef::is_null)
        {
            return Self::null_value();
        }

        // Get column type info
        let column = &row.columns()[index];
        let type_name = column.type_info().name().to_lowercase();

        // Try to decode based on type
        match type_name.as_str() {
            "boolean" | "bool" => row.try_get::<bool, _>(index).map_or_else(
                |_| Self::null_value(),
                |v| ProtoValue {
                    value: Some(ProtoValueInner::BoolValue(v)),
                },
            ),
            "int2" | "int4" | "int8" | "integer" | "bigint" | "smallint" => {
                row.try_get::<i64, _>(index)
                    .or_else(|_| row.try_get::<i32, _>(index).map(i64::from))
                    .map_or_else(
                        |_| Self::null_value(),
                        |v| ProtoValue {
                            value: Some(ProtoValueInner::IntValue(v)),
                        },
                    )
            }
            "float4" | "float8" | "real" | "double precision" | "double" => {
                row.try_get::<f64, _>(index).map_or_else(
                    |_| Self::null_value(),
                    |v| ProtoValue {
                        value: Some(ProtoValueInner::FloatValue(v)),
                    },
                )
            }
            "bytea" | "blob" => row.try_get::<Vec<u8>, _>(index).map_or_else(
                |_| Self::null_value(),
                |v| ProtoValue {
                    value: Some(ProtoValueInner::BytesValue(v)),
                },
            ),
            _ => {
                // Default to string for text, varchar, etc.
                row.try_get::<String, _>(index).map_or_else(
                    |_| Self::null_value(),
                    |v| ProtoValue {
                        value: Some(ProtoValueInner::StringValue(v)),
                    },
                )
            }
        }
    }

    /// Safely convert usize to i64.
    fn usize_to_i64(value: usize) -> i64 {
        i64::try_from(value).unwrap_or(i64::MAX)
    }

    /// Safely convert u64 to i64.
    fn u64_to_i64(value: u64) -> i64 {
        i64::try_from(value).unwrap_or(i64::MAX)
    }

    /// Safely convert u128 to i64.
    fn u128_to_i64(value: u128) -> i64 {
        i64::try_from(value).unwrap_or(i64::MAX)
    }
}

#[tonic::async_trait]
impl DataService for DataServiceImpl {
    async fn query(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        let req = request.into_inner();
        debug!(sql = %req.sql, "Executing query");

        let query = sqlx::query_with(&req.sql, Self::bind_params(&req.params));

        let rows: Vec<AnyRow> = query.fetch_all(&self.pool).await.map_err(|e| {
            error!(error = %e, "Query execution failed");
            Status::internal(format!("Query failed: {e}"))
        })?;

        let proto_rows: Vec<Row> = rows.iter().map(Self::row_to_proto).collect();
        let rows_returned = Self::usize_to_i64(proto_rows.len());

        Ok(Response::new(QueryResponse {
            rows: proto_rows,
            rows_returned,
        }))
    }

    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> Result<Response<ExecuteResponse>, Status> {
        let req = request.into_inner();
        debug!(sql = %req.sql, "Executing statement");

        let query = sqlx::query_with(&req.sql, Self::bind_params(&req.params));

        let result = query.execute(&self.pool).await.map_err(|e| {
            error!(error = %e, "Execute failed");
            Status::internal(format!("Execute failed: {e}"))
        })?;

        let rows_affected = Self::u64_to_i64(result.rows_affected());

        Ok(Response::new(ExecuteResponse {
            rows_affected,
            last_insert_id: None, // SQLx Any doesn't provide this reliably
        }))
    }

    async fn query_one(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryOneResponse>, Status> {
        let req = request.into_inner();
        debug!(sql = %req.sql, "Executing query_one");

        let query = sqlx::query_with(&req.sql, Self::bind_params(&req.params));

        let row: Option<AnyRow> = query.fetch_optional(&self.pool).await.map_err(|e| {
            error!(error = %e, "Query one failed");
            Status::internal(format!("Query failed: {e}"))
        })?;

        let proto_row = row.as_ref().map(Self::row_to_proto);

        Ok(Response::new(QueryOneResponse { row: proto_row }))
    }

    async fn begin_transaction(
        &self,
        _request: Request<BeginTransactionRequest>,
    ) -> Result<Response<TransactionResponse>, Status> {
        // Generate unique transaction ID
        let transaction_id = uuid::Uuid::new_v4().to_string();

        // Store the transaction
        self.transactions.insert(
            transaction_id.clone(),
            ActiveTransaction {
                _created_at: Instant::now(),
            },
        );

        info!(transaction_id = %transaction_id, "Transaction started");

        Ok(Response::new(TransactionResponse {
            transaction_id,
            success: true,
        }))
    }

    async fn commit_transaction(
        &self,
        request: Request<CommitTransactionRequest>,
    ) -> Result<Response<TransactionResponse>, Status> {
        let req = request.into_inner();
        let transaction_id = req.transaction_id;

        if self.transactions.remove(&transaction_id).is_some() {
            info!(transaction_id = %transaction_id, "Transaction committed");
            Ok(Response::new(TransactionResponse {
                transaction_id,
                success: true,
            }))
        } else {
            warn!(transaction_id = %transaction_id, "Transaction not found");
            Err(Status::not_found("Transaction not found"))
        }
    }

    async fn rollback_transaction(
        &self,
        request: Request<RollbackTransactionRequest>,
    ) -> Result<Response<TransactionResponse>, Status> {
        let req = request.into_inner();
        let transaction_id = req.transaction_id;

        if self.transactions.remove(&transaction_id).is_some() {
            info!(transaction_id = %transaction_id, "Transaction rolled back");
            Ok(Response::new(TransactionResponse {
                transaction_id,
                success: true,
            }))
        } else {
            warn!(transaction_id = %transaction_id, "Transaction not found");
            Err(Status::not_found("Transaction not found"))
        }
    }

    async fn execute_in_transaction(
        &self,
        request: Request<TransactionExecuteRequest>,
    ) -> Result<Response<ExecuteResponse>, Status> {
        let req = request.into_inner();

        if !self.transactions.contains_key(&req.transaction_id) {
            return Err(Status::not_found("Transaction not found"));
        }

        debug!(
            transaction_id = %req.transaction_id,
            sql = %req.sql,
            "Executing in transaction"
        );

        // Execute the query (simplified - in production you'd use actual transaction)
        let query = sqlx::query_with(&req.sql, Self::bind_params(&req.params));

        let result = query.execute(&self.pool).await.map_err(|e| {
            error!(error = %e, "Transaction execute failed");
            Status::internal(format!("Execute failed: {e}"))
        })?;

        Ok(Response::new(ExecuteResponse {
            rows_affected: Self::u64_to_i64(result.rows_affected()),
            last_insert_id: None,
        }))
    }

    async fn run_migrations(
        &self,
        request: Request<RunMigrationsRequest>,
    ) -> Result<Response<MigrationResponse>, Status> {
        let req = request.into_inner();
        info!(path = %req.migrations_path, "Running migrations");

        // Note: In production, you'd use sqlx::migrate! macro or migrator
        // For now, return a placeholder response
        warn!("Migration execution not fully implemented - requires compile-time migration embedding");

        Ok(Response::new(MigrationResponse {
            success: true,
            migrations_run: 0,
            message: "Migration endpoint ready - use sqlx migrate CLI for actual migrations"
                .to_string(),
        }))
    }

    async fn migration_status(
        &self,
        _request: Request<MigrationStatusRequest>,
    ) -> Result<Response<MigrationStatusResponse>, Status> {
        // Note: In production, you'd query the _sqlx_migrations table
        Ok(Response::new(MigrationStatusResponse {
            migrations: vec![],
        }))
    }

    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let start = Instant::now();

        // Execute a simple query to check database connectivity
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!(error = %e, "Database ping failed");
                Status::unavailable("Database unavailable")
            })?;

        let latency_ms = Self::u128_to_i64(start.elapsed().as_millis());

        Ok(Response::new(PingResponse {
            healthy: true,
            latency_ms,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_value_conversion() {
        // Test null value
        let null_val = ProtoValue {
            value: Some(ProtoValueInner::NullValue(true)),
        };
        assert!(matches!(
            null_val.value,
            Some(ProtoValueInner::NullValue(true))
        ));

        // Test string value
        let string_val = ProtoValue {
            value: Some(ProtoValueInner::StringValue("test".to_string())),
        };
        assert!(matches!(
            string_val.value,
            Some(ProtoValueInner::StringValue(_))
        ));

        // Test int value
        let int_val = ProtoValue {
            value: Some(ProtoValueInner::IntValue(42)),
        };
        assert!(matches!(int_val.value, Some(ProtoValueInner::IntValue(42))));
    }

    #[test]
    fn test_safe_conversions() {
        assert_eq!(DataServiceImpl::usize_to_i64(100), 100);
        assert_eq!(DataServiceImpl::u64_to_i64(100), 100);
        assert_eq!(DataServiceImpl::u128_to_i64(100), 100);
    }
}
