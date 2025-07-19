use axum::extract::{Query, State};
use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDateTime};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use reqwest::Client;
use rinha2025::{get_redis_connection, process, round2, AppState, PaymentsSummary, PaymentsSummaryFilter, PostPayments, SummaryData, PAYMENT_PROCESSOR_DEFAULT_URL, PAYMENT_PROCESSOR_FALLBACK_URL};
use std::env;
use std::string::String;
use std::sync::Arc;
use tokio::sync::mpsc;


#[tokio::main]
async fn main() {
    //start_service_health();
    let (tx, mut rx) = mpsc::channel::<PostPayments>(100_000);
    let tx_for_worker = tx.clone();
    let port = env::var("PORT").unwrap_or("9999".to_string());
    let client = Arc::new(Client::builder()
        //.timeout(Duration::from_millis(300))
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
            //let _ = process(payload, conn_clone, client).await;
            if let Err(e) = process(payload, conn_clone, client).await {
                eprintln!("Error processing payment: {:?}", e);
                if let Err(e) = tx_for_worker.send(post_payments).await {
                    eprintln!("❌ Failed to re-queue payment: {:?}", e);
                } else {
                    println!("🔁 Payment re-queued for retry.");
                }
            }
        }
    });


    let app = Router::new()
        .route("/payments", post(payments))
        .route("/payments-summary", get(payments_summary))
        .route("/clear_redis",get(clear_redis))
        .with_state(AppState {
            redis: Arc::clone(&connection),
            sender: tx,
        });
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn clear_redis(
    State(state): State<AppState>
) -> StatusCode {
    let mut conn = (*state.redis).clone();
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
    let mut conn = (*state.redis).clone();
    let from = date_to_ts(params.from.clone().unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()));
    let to = date_to_ts(params.to.clone().unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()));
    //println!("from: {}", from);
    //println!("to: {}", to);

    let ids_default: Vec<String> = AsyncCommands::zrangebyscore(&mut conn, "summary:default:history", from, to).await.unwrap_or_default();
    let amounts_default: Vec<f64> = AsyncCommands::hget(&mut conn,"summary:default:data", &ids_default).await.unwrap_or_default();

    let ids_fallback: Vec<String> = AsyncCommands::zrangebyscore(&mut conn,"summary:fallback:history", from, to).await.unwrap_or_default();
    let amounts_fallback: Vec<f64> = AsyncCommands::hget(&mut conn,"summary:fallback:data", &ids_fallback).await.unwrap_or_default();

    let sumary = PaymentsSummary {
        default: SummaryData {
            total_requests: amounts_default.len() as i64,
            total_amount: round2(amounts_default.iter().copied().sum()),
        },
        fallback: SummaryData {
            total_requests: amounts_fallback.len() as i64,
            total_amount: round2(amounts_fallback.iter().copied().sum()),
        },
    };
    println!("{:?}", sumary);
    /*let sumary_admin = PaymentsSummary {
        default: compare_summary(PAYMENT_PROCESSOR_DEFAULT_URL.to_string(),&params).await,
        fallback: compare_summary(PAYMENT_PROCESSOR_FALLBACK_URL.to_string(), &params).await,
    };
    println!("{:?}", sumary_admin);*/
    (StatusCode::OK, Json(sumary))
}

pub async fn compare_summary(host: String, filter: &PaymentsSummaryFilter) -> SummaryData {

    let mut querystring = Vec::new();

    if let (Some(from), Some(to)) = (&filter.from, &filter.to) {
        querystring.push(("from", from.clone()));
        querystring.push(("to", to.clone()));
    }

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-rinha-token", "123".parse().unwrap());

    let client = reqwest::Client::new();
    return client.get(format!("{}/admin/payments-summary",host))
        .query(&querystring)
        .headers(headers)
        .send()
        .await.unwrap()
        .json::<SummaryData>()
        .await
        .unwrap();
}



fn date_to_ts(date: String) -> f64 {
    if let Ok(dt) = DateTime::parse_from_rfc3339(&*date) {
        //println!("parse_from_rfc3339 {}\n", date);
        return dt.timestamp_millis() as f64;
    }
    //println!("parse_from_str {}\n", date);
    let naive = NaiveDateTime::parse_from_str(&*date, "%Y-%m-%dT%H:%M:%S").unwrap();
    naive.and_utc().timestamp_millis() as f64
}

