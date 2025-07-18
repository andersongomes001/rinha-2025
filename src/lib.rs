use redis::aio::{ConnectionManager};
use redis::{AsyncCommands, RedisError};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use chrono::{SecondsFormat, Utc};
use reqwest::Client;
use tokio::sync::mpsc::Sender;

pub const QUEUE_KEY: &str = "queue";
pub const QUEUE_FAILED_KEY: &str = "queue:failed";

#[derive(Clone)]
pub struct AppState {
    pub redis: Arc<ConnectionManager>,
    pub sender: Sender<PostPayments>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PaymentsSummaryFilter {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SummaryData {
    #[serde(rename = "totalRequests")]
    pub total_requests: i64,
    #[serde(rename = "totalAmount")]
    pub total_amount: f64,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PaymentsSummary {
    pub default: SummaryData,
    pub fallback: SummaryData,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PostPayments {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    pub amount: f64,
}

pub async fn get_redis_connection() -> Result<ConnectionManager, RedisError> {
    let client = redis::Client::open(env::var("REDIS_URL").unwrap_or("redis://127.0.0.1:6379/".to_string()))?;
    let manager = client.get_connection_manager().await?;
    Ok(manager)
}

pub async fn process(payment_json: String, mut conn: ConnectionManager, client: Arc<Client>) -> Result<(), Box<dyn std::error::Error>> {
    let processor_default_url =
        env::var("PAYMENT_PROCESSOR_DEFAULT_URL").unwrap_or("http://localhost:8001".to_string());
    let processor_fallback_url =
        env::var("PAYMENT_PROCESSOR_FALLBACK_URL").unwrap_or("http://localhost:8002".to_string());
    let payment: PostPayments = match serde_json::from_str(&payment_json) {
        Ok(p) => p,
        Err(_) => return Err(format!("Erro ao deserializar JSON {}", &payment_json).into()),
    };


    let timestamp = Utc::now();
    let timestamp_str = timestamp.to_rfc3339_opts(SecondsFormat::Millis, true);
    let timestamp_ms = timestamp.timestamp_millis() as f64;
    let payload = serde_json::json!({
        "correlationId": payment.correlation_id,
        "amount": payment.amount,
        "requestedAt" : timestamp_str
    });
    let id = format!("{}", payment.correlation_id);
    let normal_request = client
        .post(format!("{}/payments", processor_default_url))
        .json(&payload)
        .send()
        .await?;
    let mut status = normal_request.status();
    if status.is_success() {
        let _: () = conn.hset("summary:default:data", &id, payment.amount).await.unwrap();
        let _: () = conn.zadd("summary:default:history", &id, timestamp_ms).await.unwrap();
        let _: () = conn.lrem(QUEUE_FAILED_KEY, 1, &payment_json).await.unwrap();
        return Ok(());
    } else {
        let fallback_request = client
            .post(format!("{}/payments", processor_fallback_url))
            .json(&payload)
            .send()
            .await?;
        status = fallback_request.status();
        if status.is_success() {
            let _: () = conn.hset("summary:fallback:data", &id, payment.amount).await.unwrap();
            let _: () = conn.zadd("summary:fallback:history", &id, timestamp_ms).await.unwrap();
            let _: () = conn.lrem( QUEUE_FAILED_KEY, 1, &payment_json).await.unwrap();
            return Ok(());
        }
    }
    let _: () = conn.rpush(QUEUE_FAILED_KEY, payment_json).await.unwrap();
    Err(format!("Erro ao enviar todas as requisiçoes {:?}", status).into())
}
