use std::env;
use std::sync::atomic::AtomicBool;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::{RwLock};
use crate::domain::entities::HealthStatusAll;
use crate::HealthResponse;

pub const QUEUE_KEY: &str = "queue";
pub const QUEUE_FAILED_KEY: &str = "queue:failed";
pub static HEALTH_STATUS: Lazy<AtomicBool> = Lazy::new(||AtomicBool::new(true));

pub static GLOBAL_HEALTH_STATUS: Lazy<Arc<RwLock<HealthStatusAll>>> = Lazy::new(|| {
    Arc::new(RwLock::new(HealthStatusAll {
        default: HealthResponse {
            failing: false,
            min_response_time: 0,
            failing_since: None
        },
        fallback: HealthResponse {
            failing: false,
            min_response_time: 5000,
            failing_since: None
        }
    }))
});
pub static PAYMENT_PROCESSOR_DEFAULT_URL: Lazy<String> = Lazy::new(|| {
    env::var("PAYMENT_PROCESSOR_DEFAULT_URL").unwrap_or_else(|_| "http://localhost:8001".to_string())
});
pub static PAYMENT_PROCESSOR_FALLBACK_URL: Lazy<String> = Lazy::new(|| {
    env::var("PAYMENT_PROCESSOR_FALLBACK_URL").unwrap_or_else(|_| "http://localhost:8002".to_string())
});

pub static REDIS_URL: Lazy<String> = Lazy::new(|| {
    env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string())
});

pub static INSTANCE_ROLE: Lazy<String> = Lazy::new(|| {
    env::var("INSTANCE_ROLE").unwrap_or_else(|_| "none".to_string())
});

pub static WS_MASTER_URL: Lazy<String> = Lazy::new(|| {
    env::var("WS_MASTER_URL").unwrap_or_else(|_| "ws://127.0.0.1:9001".to_string())
});
