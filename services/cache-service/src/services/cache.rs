//! Cache service gRPC implementation.

use acton_dx_proto::cache::v1::{
    cache_service_server::CacheService, DeleteRequest, DeleteResponse, ExistsRequest,
    ExistsResponse, GetRequest, GetResponse, HGetAllRequest, HGetAllResponse, HGetRequest,
    HGetResponse, HSetRequest, HSetResponse, IncrementRequest, IncrementResponse, LPushRequest,
    LPushResponse, LRangeRequest, LRangeResponse, RPopRequest, RPopResponse, RateLimitRequest,
    RateLimitResponse, SetRequest, SetResponse,
};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tonic::{Request, Response, Status};
use tracing::{debug, error};

/// Cache service implementation.
pub struct CacheServiceImpl {
    /// Redis connection manager.
    conn: ConnectionManager,
}

impl CacheServiceImpl {
    /// Create a new cache service with the given Redis connection.
    #[must_use]
    pub const fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }

    /// Get current unix timestamp.
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }

    /// Safely convert i64 to i32.
    fn i64_to_i32(value: i64) -> i32 {
        i32::try_from(value).unwrap_or(i32::MAX)
    }

    /// Safely convert i64 to isize.
    fn i64_to_isize(value: i64) -> isize {
        isize::try_from(value).unwrap_or(isize::MAX)
    }
}

#[tonic::async_trait]
impl CacheService for CacheServiceImpl {
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, "GET");

        let mut conn = self.conn.clone();
        let result: Option<Vec<u8>> = conn.get(&req.key).await.map_err(|e| {
            error!(error = %e, key = %req.key, "GET failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        Ok(Response::new(GetResponse {
            found: result.is_some(),
            value: result,
        }))
    }

    async fn set(&self, request: Request<SetRequest>) -> Result<Response<SetResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, ttl = ?req.ttl_seconds, "SET");

        let mut conn = self.conn.clone();

        if let Some(ttl) = req.ttl_seconds {
            let ttl_u64 = u64::try_from(ttl).unwrap_or(u64::MAX);
            conn.set_ex::<_, _, ()>(&req.key, &req.value, ttl_u64)
                .await
                .map_err(|e| {
                    error!(error = %e, key = %req.key, "SET failed");
                    Status::internal(format!("Redis error: {e}"))
                })?;
        } else {
            conn.set::<_, _, ()>(&req.key, &req.value)
                .await
                .map_err(|e| {
                    error!(error = %e, key = %req.key, "SET failed");
                    Status::internal(format!("Redis error: {e}"))
                })?;
        }

        Ok(Response::new(SetResponse { success: true }))
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, "DELETE");

        let mut conn = self.conn.clone();
        let deleted: i64 = conn.del(&req.key).await.map_err(|e| {
            error!(error = %e, key = %req.key, "DELETE failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        Ok(Response::new(DeleteResponse { deleted: deleted > 0 }))
    }

    async fn exists(
        &self,
        request: Request<ExistsRequest>,
    ) -> Result<Response<ExistsResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, "EXISTS");

        let mut conn = self.conn.clone();
        let exists: bool = conn.exists(&req.key).await.map_err(|e| {
            error!(error = %e, key = %req.key, "EXISTS failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        Ok(Response::new(ExistsResponse { exists }))
    }

    async fn check_rate_limit(
        &self,
        request: Request<RateLimitRequest>,
    ) -> Result<Response<RateLimitResponse>, Status> {
        let req = request.into_inner();
        debug!(
            key = %req.key,
            limit = req.limit,
            window = req.window_seconds,
            "CHECK_RATE_LIMIT"
        );

        let mut conn = self.conn.clone();
        let now = Self::current_timestamp();
        let window_start = now - u64::from(req.window_seconds.unsigned_abs());

        // Use sorted set for sliding window rate limiting
        let rate_key = format!("ratelimit:{}", req.key);

        // Remove old entries
        conn.zrembyscore::<_, _, _, ()>(&rate_key, 0_u64, window_start)
            .await
            .map_err(|e| {
                error!(error = %e, "ZREMRANGEBYSCORE failed");
                Status::internal(format!("Redis error: {e}"))
            })?;

        // Count current entries
        let count: i64 = conn.zcard(&rate_key).await.map_err(|e| {
            error!(error = %e, "ZCARD failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        let limit_i64 = i64::from(req.limit);
        let allowed = count < limit_i64;
        let remaining = Self::i64_to_i32((limit_i64 - count - 1).max(0));
        let reset_in = req.window_seconds;

        if allowed {
            // Add current request
            conn.zadd::<_, _, _, ()>(&rate_key, now, format!("{now}:{count}"))
                .await
                .map_err(|e| {
                    error!(error = %e, "ZADD failed");
                    Status::internal(format!("Redis error: {e}"))
                })?;

            // Set expiry on the key
            let window_i64 = i64::from(req.window_seconds);
            conn.expire::<_, ()>(&rate_key, window_i64)
                .await
                .map_err(|e| {
                    error!(error = %e, "EXPIRE failed");
                    Status::internal(format!("Redis error: {e}"))
                })?;
        }

        Ok(Response::new(RateLimitResponse {
            allowed,
            remaining,
            reset_in_seconds: reset_in,
        }))
    }

    async fn increment_counter(
        &self,
        request: Request<IncrementRequest>,
    ) -> Result<Response<IncrementResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, amount = req.amount, "INCREMENT");

        let mut conn = self.conn.clone();
        let new_value: i64 = conn.incr(&req.key, req.amount).await.map_err(|e| {
            error!(error = %e, key = %req.key, "INCRBY failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        if let Some(ttl) = req.ttl_seconds {
            conn.expire::<_, ()>(&req.key, ttl).await.map_err(|e| {
                error!(error = %e, key = %req.key, "EXPIRE failed");
                Status::internal(format!("Redis error: {e}"))
            })?;
        }

        Ok(Response::new(IncrementResponse { new_value }))
    }

    async fn h_get(&self, request: Request<HGetRequest>) -> Result<Response<HGetResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, field = %req.field, "HGET");

        let mut conn = self.conn.clone();
        let result: Option<Vec<u8>> = conn.hget(&req.key, &req.field).await.map_err(|e| {
            error!(error = %e, key = %req.key, "HGET failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        Ok(Response::new(HGetResponse {
            found: result.is_some(),
            value: result,
        }))
    }

    async fn h_set(&self, request: Request<HSetRequest>) -> Result<Response<HSetResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, field = %req.field, "HSET");

        let mut conn = self.conn.clone();
        conn.hset::<_, _, _, ()>(&req.key, &req.field, &req.value)
            .await
            .map_err(|e| {
                error!(error = %e, key = %req.key, "HSET failed");
                Status::internal(format!("Redis error: {e}"))
            })?;

        Ok(Response::new(HSetResponse { success: true }))
    }

    async fn h_get_all(
        &self,
        request: Request<HGetAllRequest>,
    ) -> Result<Response<HGetAllResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, "HGETALL");

        let mut conn = self.conn.clone();
        let result: HashMap<String, Vec<u8>> = conn.hgetall(&req.key).await.map_err(|e| {
            error!(error = %e, key = %req.key, "HGETALL failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        Ok(Response::new(HGetAllResponse { fields: result }))
    }

    async fn l_push(
        &self,
        request: Request<LPushRequest>,
    ) -> Result<Response<LPushResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, "LPUSH");

        let mut conn = self.conn.clone();
        let length: i64 = conn.lpush(&req.key, &req.value).await.map_err(|e| {
            error!(error = %e, key = %req.key, "LPUSH failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        Ok(Response::new(LPushResponse { length }))
    }

    async fn r_pop(&self, request: Request<RPopRequest>) -> Result<Response<RPopResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, "RPOP");

        let mut conn = self.conn.clone();
        let value: Option<Vec<u8>> = conn.rpop(&req.key, None).await.map_err(|e| {
            error!(error = %e, key = %req.key, "RPOP failed");
            Status::internal(format!("Redis error: {e}"))
        })?;

        Ok(Response::new(RPopResponse { value }))
    }

    async fn l_range(
        &self,
        request: Request<LRangeRequest>,
    ) -> Result<Response<LRangeResponse>, Status> {
        let req = request.into_inner();
        debug!(key = %req.key, start = req.start, stop = req.stop, "LRANGE");

        let mut conn = self.conn.clone();
        let start = Self::i64_to_isize(req.start);
        let stop = Self::i64_to_isize(req.stop);
        let values: Vec<Vec<u8>> = conn
            .lrange(&req.key, start, stop)
            .await
            .map_err(|e| {
                error!(error = %e, key = %req.key, "LRANGE failed");
                Status::internal(format!("Redis error: {e}"))
            })?;

        Ok(Response::new(LRangeResponse { values }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp() {
        let ts = CacheServiceImpl::current_timestamp();
        assert!(ts > 0);
    }

    #[test]
    fn test_i64_to_i32() {
        assert_eq!(CacheServiceImpl::i64_to_i32(100), 100);
        assert_eq!(CacheServiceImpl::i64_to_i32(i64::MAX), i32::MAX);
    }
}
