use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use axum::body::Bytes;
use tokio::sync::mpsc::Sender;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct HealthResponse {
    pub failing : bool,
    #[serde(rename = "minResponseTime")]
    pub min_response_time: i64,
    pub failing_since: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug,Clone)]
pub struct HealthStatusAll {
    pub default: HealthResponse,
    pub fallback: HealthResponse,
}

#[derive(Clone)]
pub struct AppState {
    pub redis: Arc<ConnectionManager>,
    pub sender: Sender<Bytes>
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

#[derive(Deserialize, Serialize,Debug, PartialEq)]
pub enum ProcessorDecision {
    DEFAULT,
    FALLBACK,
    FAILING,
}
