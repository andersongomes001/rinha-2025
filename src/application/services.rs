use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use crate::infrastructure::config::{PAYMENT_PROCESSOR_DEFAULT_URL, PAYMENT_PROCESSOR_FALLBACK_URL, QUEUE_FAILED_KEY};
use crate::infrastructure::{payments_request, store_summary};
use chrono::{SecondsFormat, Utc};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use reqwest::Client;
use crate::{AnyError, PostPayments};

pub async fn process(payment_json: String, conn: Arc<ConnectionManager>, client: Arc<Client>) -> Result<(), AnyError> {
    let mut conn = (*conn).clone();
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

    for _ in 0..5 {
        let normal_request = payments_request(&client, PAYMENT_PROCESSOR_DEFAULT_URL.as_str().parse().unwrap(), &payload).await?;
        let status = normal_request.status();
        if status.is_success() {
            store_summary(&mut conn, "default", &id, payment.amount, timestamp_ms, &payment_json).await?;
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let fallback_request = payments_request(&client, PAYMENT_PROCESSOR_FALLBACK_URL.as_str().parse().unwrap(), &payload).await?;
    let status = fallback_request.status();
    if status.is_success() {
        store_summary(&mut conn, "fallback", &id, payment.amount, timestamp_ms, &payment_json).await?;
        return Ok(());
    }
    //if HEALTH_STATUS.load(Ordering::Relaxed) {}
    //let _ = conn.rpush::<_, _, String>(QUEUE_FAILED_KEY, payment_json).await;
    Err("Erro ao enviar todas as requisiçoes".to_string().into())
}

