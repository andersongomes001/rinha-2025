use crate::domain::entities::{AppState, PaymentsSummary, PaymentsSummaryFilter, SummaryData};
use crate::infrastructure::{date_to_ts, round2};
use crate::PostPayments;
use axum::extract::{Query, State};
use axum::{
    http::StatusCode,
    Json,
};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::string::String;

pub async fn clear_redis(
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

pub async fn payments(
    State(state): State<AppState>,
    Json(payload): Json<PostPayments>,
) -> StatusCode {
    match state.sender.try_send(payload) {
        Ok(_) => StatusCode::CREATED,
        Err(_) =>  StatusCode::TOO_MANY_REQUESTS,
    }
}

pub async fn payments_summary(
    Query(params): Query<PaymentsSummaryFilter>,
    State(state): State<AppState>,
) -> (StatusCode, Json<PaymentsSummary>) {
    let mut conn = (*state.redis).clone();
    let mut summary = PaymentsSummary {
        default: SummaryData {
            total_requests: 0,
            total_amount: 0.0,
        },
        fallback: SummaryData {
            total_requests: 0,
            total_amount: 0.0,
        }
    };
    if params.from.is_none() && params.to.is_none() {
        return (StatusCode::OK, Json(summary));
    }

    let from = date_to_ts(params.from.as_deref().unwrap_or("1970-01-01T00:00:00Z").parse().unwrap());
    let to = date_to_ts(params.to.as_deref().unwrap_or("1970-01-01T00:00:00Z").parse().unwrap());

    let (amounts_default, amounts_fallback) = get_summary_data(&mut conn, from, to).await;

    summary = PaymentsSummary {
        default: SummaryData {
            total_requests: amounts_default.len() as i64,
            total_amount: round2(amounts_default.iter().sum::<f64>()),
        },
        fallback: SummaryData {
            total_requests: amounts_fallback.len() as i64,
            total_amount: round2(amounts_fallback.iter().sum::<f64>()),
        },
    };

    (StatusCode::OK, Json(summary))

    /*let mut conn = (*state.redis).clone();
    let from = date_to_ts(params.from.clone().unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()));
    let to = date_to_ts(params.to.clone().unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()));

    let (amounts_default, amounts_fallback) = get_summary_data(&mut conn, from, to).await;

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
    (StatusCode::OK, Json(sumary))*/
}

async fn get_summary_data(conn: &mut ConnectionManager, from: f64, to: f64) -> (Vec<f64>, Vec<f64>) {
    //println!("{}", to);
    let ids_default: Vec<String> = AsyncCommands::zrangebyscore(conn, "summary:default:history", from, to).await.unwrap_or_default();
    let amounts_default: Vec<f64> = AsyncCommands::hget(conn, "summary:default:data", &ids_default).await.unwrap_or_default();

    let ids_fallback: Vec<String> = AsyncCommands::zrangebyscore(conn, "summary:fallback:history", from, to).await.unwrap_or_default();
    let amounts_fallback: Vec<f64> = AsyncCommands::hget(conn, "summary:fallback:data", &ids_fallback).await.unwrap_or_default();
    (amounts_default, amounts_fallback)
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
