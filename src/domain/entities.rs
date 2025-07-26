use std::sync::Arc;
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct HealthResponse {
    pub failing : bool,
    #[serde(rename = "minResponseTime")]
    pub min_response_time: i64
}

#[derive(Deserialize, Serialize, Debug)]
pub struct HealthStatusAll {
    pub default: HealthResponse,
    pub fallback: HealthResponse,
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

