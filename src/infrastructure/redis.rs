use redis::aio::ConnectionManager;
use redis::{pipe, RedisError};
use crate::infrastructure::config::{REDIS_URL};

pub async fn get_redis_connection() -> Result<ConnectionManager, RedisError> {
    let client = redis::Client::open(REDIS_URL.as_str().to_string())?;   //redis::Client::open(env::var("REDIS_URL").unwrap_or("redis://127.0.0.1:6379/".to_string()))?;
    let manager = client.get_connection_manager().await?;
    Ok(manager)
}


pub async fn store_summary(conn: &mut ConnectionManager, key_prefix: &str, id: &str, amount: f64, timestamp_ms: f64) -> redis::RedisResult<()> {
    //println!("store_summary => key_prefix: {}, id: {}, amount: {}, timestamp: {}", key_prefix, id, amount, timestamp_ms);
    pipe()
        .atomic()
        .hset(format!("summary:{}:data", key_prefix), id, amount)
        .zadd(format!("summary:{}:history", key_prefix), id, timestamp_ms)
        .query_async(conn)
        .await
}
