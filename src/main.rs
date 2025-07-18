use axum::extract::{Query, State};
use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDateTime};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands};
use reqwest::Client;
use rinha2025::{get_redis_connection, process, AppState, PaymentsSummary, PaymentsSummaryFilter, PostPayments, SummaryData};
use std::env;
use std::string::String;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<PostPayments>(100_000);
    let port = env::var("PORT").unwrap_or("9999".to_string());
    let client = Arc::new(Client::builder()
        .timeout(Duration::from_millis(300))
        .build()
        .unwrap());

    let connection : Arc<ConnectionManager> = match get_redis_connection().await {
        Ok(conn) => Arc::new(conn),
        Err(e) => {
            eprintln!("Failed to connect to Redis: {:?}", e);
            return;
        }
    };

    let connection_for_worker = Arc::clone(&connection);
    tokio::spawn(async move {
        while let Some(post_payments) = rx.recv().await {
            let client = Arc::clone(&client);
            let conn_clone = (*connection_for_worker).clone();
            let payload = serde_json::to_string(&post_payments).unwrap();
            let _ = process(payload, conn_clone, client).await;
        }
    });


    let app = Router::new()
        .route("/payments", post(payments))
        .route("/payments-summary", get(payments_summary))
        .route("/clear_redis",get(clear_redis))
        .with_state(AppState {
            redis: Arc::clone(&connection),
            sender: tx.clone(),
        });
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn clear_redis(
    State(state): State<AppState>
) -> StatusCode {
    let mut conn = Arc::try_unwrap(state.redis.clone()).unwrap_or_else(|arc| (*arc).clone());
    match AsyncCommands::flushall::<String>(&mut conn).await {
        Ok(_) => {
            StatusCode::OK
        },
        Err(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn payments(
    State(state): State<AppState>,
    Json(payload): Json<PostPayments>,
) -> StatusCode {
    match state.sender.send(payload).await {
        Ok(_) => {
            StatusCode::CREATED
        },
        Err(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn payments_summary(
    Query(params): Query<PaymentsSummaryFilter>,
    State(state): State<AppState>,
) -> (StatusCode, Json<PaymentsSummary>) {
    let mut conn = Arc::try_unwrap(state.redis.clone()).unwrap_or_else(|arc| (*arc).clone());
    let from = date_to_ts(params.from);
    let to = date_to_ts(params.to);
    println!("from: {}", from);
    println!("to: {}", to);

    let ids_default: Vec<String> = AsyncCommands::zrangebyscore(&mut conn, "summary:default:history", from, to).await.unwrap_or_default();
    let amounts_default: Vec<f64> = AsyncCommands::hget(&mut conn,"summary:default:data", &ids_default).await.unwrap_or_default();

    let ids_fallback: Vec<String> = AsyncCommands::zrangebyscore(&mut conn,"summary:fallback:history", from, to).await.unwrap_or_default();
    let amounts_fallback: Vec<f64> = AsyncCommands::hget(&mut conn,"summary:fallback:data", &ids_fallback).await.unwrap_or_default();

    let sumary = PaymentsSummary {
        default: SummaryData {
            total_requests: amounts_default.len() as i64,
            total_amount: amounts_default.iter().copied().sum(),
        },
        fallback: SummaryData {
            total_requests: amounts_fallback.len() as i64,
            total_amount: amounts_fallback.iter().copied().sum(),
        },
    };
    println!("{:?}", sumary);

    (StatusCode::OK, Json(sumary))
}


fn date_to_ts(date: String) -> i64 {
    if let Ok(dt) = DateTime::parse_from_rfc3339(&*date) {
        return dt.timestamp_millis();
    }
    let naive = NaiveDateTime::parse_from_str(&*date, "%Y-%m-%dT%H:%M:%S").unwrap();
    naive.and_utc().timestamp_millis()
}

