use crate::infrastructure::config::{PAYMENT_PROCESSOR_DEFAULT_URL, PAYMENT_PROCESSOR_FALLBACK_URL};
use crate::infrastructure::{payments_request, store_summary};
use crate::{AnyError, PostPayments};
use chrono::{SecondsFormat, Utc};
use redis::aio::ConnectionManager;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use crate::domain::entities::ProcessorDecision;

pub async fn process(payment_json: String, conn: Arc<ConnectionManager>, client: Arc<Client>, decision: ProcessorDecision) -> Result<(), AnyError> {
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
    let mut need_fallback = decision == ProcessorDecision::FALLBACK;

    if decision == ProcessorDecision::DEFAULT {
        for _ in 0..2 {
            let resp = payments_request(&client, PAYMENT_PROCESSOR_DEFAULT_URL.as_str().parse().unwrap(), &payload).await?;
            if resp.status().is_success() {
                store_summary(&mut conn, "default", &id, payment.amount, timestamp_ms).await?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        need_fallback = true;
    }

    if need_fallback {
        let resp = payments_request(&client, PAYMENT_PROCESSOR_FALLBACK_URL.as_str().parse().unwrap(), &payload).await?;
        if resp.status().is_success() {
            store_summary(&mut conn, "fallback", &id, payment.amount, timestamp_ms).await?;
            return Ok(());
        }
    }

    Err("Erro ao enviar todas as requisiçoes".to_string().into())
}

