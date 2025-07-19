use std::env;
use std::sync::atomic::AtomicBool;
use once_cell::sync::Lazy;

pub const QUEUE_KEY: &str = "queue";
pub const QUEUE_FAILED_KEY: &str = "queue:failed";
pub static HEALTH_STATUS: Lazy<AtomicBool> = Lazy::new(||AtomicBool::new(true));


pub static PAYMENT_PROCESSOR_DEFAULT_URL: Lazy<String> = Lazy::new(|| {
    env::var("PAYMENT_PROCESSOR_DEFAULT_URL").unwrap_or_else(|_| "http://localhost:8001".to_string())
});
pub static PAYMENT_PROCESSOR_FALLBACK_URL: Lazy<String> = Lazy::new(|| {
    env::var("PAYMENT_PROCESSOR_FALLBACK_URL").unwrap_or_else(|_| "http://localhost:8002".to_string())
});

pub static REDIS_URL: Lazy<String> = Lazy::new(|| {
    env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string())
});
