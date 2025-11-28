//! Cache service client for Redis operations.

use super::error::ClientError;
use acton_dx_proto::cache::v1::{
    cache_service_client::CacheServiceClient, DeleteRequest, ExistsRequest, GetRequest,
    HGetAllRequest, HGetRequest, HSetRequest, IncrementRequest, LPushRequest, LRangeRequest,
    RPopRequest, RateLimitRequest, SetRequest,
};
use std::collections::HashMap;
use tonic::transport::Channel;

/// Client for the cache service.
///
/// Provides Redis operations including key-value storage, rate limiting,
/// hash operations, and list operations.
#[derive(Debug, Clone)]
pub struct CacheClient {
    client: CacheServiceClient<Channel>,
}

impl CacheClient {
    /// Connect to the cache service.
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
            client: CacheServiceClient::new(channel),
        })
    }

    // ==================== Key-Value Operations ====================

    /// Get a value by key.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, ClientError> {
        let response = self
            .client
            .get(GetRequest {
                key: key.to_string(),
            })
            .await?;

        let inner = response.into_inner();
        if inner.found {
            Ok(inner.value)
        } else {
            Ok(None)
        }
    }

    /// Get a string value by key.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails or value is not valid UTF-8.
    pub async fn get_string(&mut self, key: &str) -> Result<Option<String>, ClientError> {
        let value = self.get(key).await?;
        value
            .map(|v| {
                String::from_utf8(v)
                    .map_err(|e| ClientError::ResponseError(format!("Invalid UTF-8: {e}")))
            })
            .transpose()
    }

    /// Set a value.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn set(
        &mut self,
        key: &str,
        value: &[u8],
        ttl_seconds: Option<i64>,
    ) -> Result<bool, ClientError> {
        let response = self
            .client
            .set(SetRequest {
                key: key.to_string(),
                value: value.to_vec(),
                ttl_seconds,
            })
            .await?;

        Ok(response.into_inner().success)
    }

    /// Set a string value.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn set_string(
        &mut self,
        key: &str,
        value: &str,
        ttl_seconds: Option<i64>,
    ) -> Result<bool, ClientError> {
        self.set(key, value.as_bytes(), ttl_seconds).await
    }

    /// Delete a key.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn delete(&mut self, key: &str) -> Result<bool, ClientError> {
        let response = self
            .client
            .delete(DeleteRequest {
                key: key.to_string(),
            })
            .await?;

        Ok(response.into_inner().deleted)
    }

    /// Check if a key exists.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn exists(&mut self, key: &str) -> Result<bool, ClientError> {
        let response = self
            .client
            .exists(ExistsRequest {
                key: key.to_string(),
            })
            .await?;

        Ok(response.into_inner().exists)
    }

    // ==================== Rate Limiting ====================

    /// Check if an action is within rate limits.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn check_rate_limit(
        &mut self,
        key: &str,
        limit: i32,
        window_seconds: i32,
    ) -> Result<RateLimitResult, ClientError> {
        let response = self
            .client
            .check_rate_limit(RateLimitRequest {
                key: key.to_string(),
                limit,
                window_seconds,
            })
            .await?;

        let inner = response.into_inner();
        Ok(RateLimitResult {
            allowed: inner.allowed,
            remaining: inner.remaining,
            reset_in_seconds: inner.reset_in_seconds,
        })
    }

    /// Increment a counter.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn increment(
        &mut self,
        key: &str,
        amount: i64,
        ttl_seconds: Option<i64>,
    ) -> Result<i64, ClientError> {
        let response = self
            .client
            .increment_counter(IncrementRequest {
                key: key.to_string(),
                amount,
                ttl_seconds,
            })
            .await?;

        Ok(response.into_inner().new_value)
    }

    // ==================== Hash Operations ====================

    /// Get a hash field value.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn hget(&mut self, key: &str, field: &str) -> Result<Option<Vec<u8>>, ClientError> {
        let response = self
            .client
            .h_get(HGetRequest {
                key: key.to_string(),
                field: field.to_string(),
            })
            .await?;

        let inner = response.into_inner();
        if inner.found {
            Ok(inner.value)
        } else {
            Ok(None)
        }
    }

    /// Set a hash field value.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn hset(
        &mut self,
        key: &str,
        field: &str,
        value: &[u8],
    ) -> Result<bool, ClientError> {
        let response = self
            .client
            .h_set(HSetRequest {
                key: key.to_string(),
                field: field.to_string(),
                value: value.to_vec(),
            })
            .await?;

        Ok(response.into_inner().success)
    }

    /// Get all hash fields.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn hgetall(&mut self, key: &str) -> Result<HashMap<String, Vec<u8>>, ClientError> {
        let response = self
            .client
            .h_get_all(HGetAllRequest {
                key: key.to_string(),
            })
            .await?;

        Ok(response.into_inner().fields)
    }

    // ==================== List Operations ====================

    /// Push a value to the left of a list.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn lpush(&mut self, key: &str, value: &[u8]) -> Result<i64, ClientError> {
        let response = self
            .client
            .l_push(LPushRequest {
                key: key.to_string(),
                value: value.to_vec(),
            })
            .await?;

        Ok(response.into_inner().length)
    }

    /// Pop a value from the right of a list.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn rpop(&mut self, key: &str) -> Result<Option<Vec<u8>>, ClientError> {
        let response = self
            .client
            .r_pop(RPopRequest {
                key: key.to_string(),
            })
            .await?;

        Ok(response.into_inner().value)
    }

    /// Get a range of list elements.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn lrange(
        &mut self,
        key: &str,
        start: i64,
        stop: i64,
    ) -> Result<Vec<Vec<u8>>, ClientError> {
        let response = self
            .client
            .l_range(LRangeRequest {
                key: key.to_string(),
                start,
                stop,
            })
            .await?;

        Ok(response.into_inner().values)
    }
}

/// Result of a rate limit check.
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the action is allowed.
    pub allowed: bool,
    /// Number of remaining requests in the window.
    pub remaining: i32,
    /// Seconds until the rate limit resets.
    pub reset_in_seconds: i32,
}
