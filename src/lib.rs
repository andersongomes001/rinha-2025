use chrono::{SecondsFormat, Utc};
use once_cell::sync::Lazy;
use redis::aio::ConnectionManager;
use redis::{pipe, AsyncCommands, RedisError};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

pub const QUEUE_KEY: &str = "queue";
pub const QUEUE_FAILED_KEY: &str = "queue:failed";
pub static HEALTH_STATUS: Lazy<AtomicBool> = Lazy::new(||AtomicBool::new(true));


pub static PAYMENT_PROCESSOR_DEFAULT_URL: Lazy<String> = Lazy::new(|| {
    env::var("PAYMENT_PROCESSOR_DEFAULT_URL").unwrap_or_else(|_| "http://localhost:8001".to_string())
});
pub static PAYMENT_PROCESSOR_FALLBACK_URL: Lazy<String> = Lazy::new(|| {
    env::var("PAYMENT_PROCESSOR_FALLBACK_URL").unwrap_or_else(|_| "http://localhost:8002".to_string())
});

#[derive(Deserialize, Serialize, Debug)]
pub struct HealthResponse {
    pub failing : bool,
    #[serde(rename = "minResponseTime")]
    pub min_response_time: i64
}

#[derive(Clone)]
pub struct AppState {
    pub redis: Arc<ConnectionManager>,
    pub sender: Sender<PostPayments>,
}

#[derive(Deserialize, Serialize, Debug,Clone)]
pub struct PaymentsSummaryFilter {
    pub from: Option<String>,
    pub to: Option<String>,
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

pub type AnyError = Box<dyn std::error::Error + Send + Sync>;

pub async fn get_redis_connection() -> Result<ConnectionManager, RedisError> {
    let client = redis::Client::open(env::var("REDIS_URL").unwrap_or("redis://127.0.0.1:6379/".to_string()))?;
    let manager = client.get_connection_manager().await?;
    Ok(manager)
}

pub async fn process(payment_json: String, mut conn: ConnectionManager, client: Arc<Client>) -> Result<(), AnyError> {
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
    let normal_request = payments_request(&client, PAYMENT_PROCESSOR_DEFAULT_URL.as_str().parse().unwrap(), &payload).await?;
    let status = normal_request.status();
    if status.is_success() {
        store_summary(&mut conn, "default", &id, payment.amount, timestamp_ms, &payment_json).await?;
        return Ok(());
    } else {
        let fallback_request = payments_request(&client, PAYMENT_PROCESSOR_FALLBACK_URL.as_str().parse().unwrap(), &payload).await?;
        let status = fallback_request.status();
        if status.is_success() {
            store_summary(&mut conn, "fallback", &id, payment.amount, timestamp_ms, &payment_json).await?;
            return Ok(());
        }
    }
    //if HEALTH_STATUS.load(Ordering::Relaxed) {}
    let _ = conn.rpush::<_, _, String>(QUEUE_FAILED_KEY, payment_json).await;
    Err("Erro ao enviar todas as requisiçoes".to_string().into())
}

async fn payments_request(client: &Arc<Client>, host: String, payload: &Value) -> Result<Response, reqwest::Error> {
    client
        .post(format!("{}/payments", host))
        .json(&payload)
        .send().await
}

pub async fn store_summary(conn: &mut ConnectionManager, key_prefix: &str, id: &str, amount: f64, timestamp_ms: f64, json: &str) -> redis::RedisResult<()> {
    //println!("store_summary => key_prefix: {}, id: {}, amount: {}, timestamp: {}", key_prefix, id, amount, timestamp_ms);
    pipe()
        .atomic()
        .hset(format!("summary:{}:data", key_prefix), id, amount)
        .zadd(format!("summary:{}:history", key_prefix), id, timestamp_ms)
        .lrem(QUEUE_FAILED_KEY, 1, json)
        .query_async(conn)
        .await
}


pub fn start_service_health() {
    tokio::spawn(async move {
        loop {
            let result = Client::builder()
                .timeout(std::time::Duration::from_millis(10))
                .build()
                .unwrap()
                .get(format!("{}/payments/service-health",PAYMENT_PROCESSOR_DEFAULT_URL.to_string()))
                .send()
                .await;
            match result {
                Ok(response) => {
                    if let Ok(json) = response.json::<HealthResponse>().await {
                        HEALTH_STATUS.store(!json.failing, Ordering::Relaxed);
                    } else {
                        HEALTH_STATUS.store(false, Ordering::Relaxed);
                    }

                }
                Err(_) => {
                    HEALTH_STATUS.store(false, Ordering::Relaxed);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}


pub fn round2(val: f64) -> f64 {
    let rounded = (val * 100.0).round() / 100.0;
    sanitize_zero(rounded)
}

//remove os -0.0 para ficar 0.0
fn sanitize_zero(val: f64) -> f64 {
    if val == 0.0 {
        0.0
    } else {
        val
    }
}
