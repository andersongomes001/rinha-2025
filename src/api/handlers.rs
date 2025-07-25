use crate::infrastructure::{date_to_ts, round2};
use axum::extract::{Query, State};
use axum::{
    http::StatusCode
    ,
    Json,
};
use redis::AsyncCommands;
use std::string::String;
use std::sync::Arc;
use std::time::Instant;
use redis::aio::ConnectionManager;
use crate::domain::entities::{AppState, PaymentsSummary, PaymentsSummaryFilter, SummaryData};
use crate::PostPayments;

pub async fn clear_redis(redis: Arc<ConnectionManager> ){
    let mut conn = (*redis).clone();
    match AsyncCommands::flushall::<String>(&mut conn).await {
        Ok(_) => {
            println!("Redis cache cleared successfully.");
        },
        Err(_) => {
            println!("Failed to clear Redis cache.");
        }
    }
}
/*pub async fn clear_redis(
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
}*/

pub async fn payments(
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

pub async fn payments_summary(
    Query(params): Query<PaymentsSummaryFilter>,
    State(state): State<AppState>,
) -> (StatusCode, Json<PaymentsSummary>) {
    let mut conn = (*state.redis).clone();
    let from = date_to_ts(params.from.clone().unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()));
    let to = date_to_ts(params.to.clone().unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()));
    //println!("from: {}", from);
    //println!("to: {}", to);
    let start = Instant::now();
    let ids_default: Vec<String> = AsyncCommands::zrangebyscore(&mut conn, "summary:default:history", from, to).await.unwrap_or_default();
    let amounts_default: Vec<f64> = AsyncCommands::hget(&mut conn,"summary:default:data", &ids_default).await.unwrap_or_default();

    let ids_fallback: Vec<String> = AsyncCommands::zrangebyscore(&mut conn,"summary:fallback:history", from, to).await.unwrap_or_default();
    let amounts_fallback: Vec<f64> = AsyncCommands::hget(&mut conn,"summary:fallback:data", &ids_fallback).await.unwrap_or_default();
    println!("Summary gerado em {:?}", start.elapsed());

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
