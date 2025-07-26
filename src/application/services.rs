use crate::infrastructure::config::{
    GLOBAL_HEALTH_STATUS, PAYMENT_PROCESSOR_DEFAULT_URL, PAYMENT_PROCESSOR_FALLBACK_URL,
    QUEUE_FAILED_KEY,
};
use crate::infrastructure::{payments_request, store_summary};
use crate::{AnyError, PostPayments};
use chrono::{SecondsFormat, Utc};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use reqwest::Client;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

pub async fn process(
    payment_json: String,
    conn: Arc<ConnectionManager>,
    client: Arc<Client>,
) -> Result<(), AnyError> {
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
    let rota = escolher_rota_async(0.1).await;

    match rota {
        "default" => {
            let normal_request = payments_request(
                &client,
                PAYMENT_PROCESSOR_DEFAULT_URL.as_str().parse().unwrap(),
                &payload,
            )
            .await?;
            let status = normal_request.status();
            if status.is_success() {
                store_summary(
                    &mut conn,
                    "default",
                    &id,
                    payment.amount,
                    timestamp_ms,
                    &payment_json,
                )
                .await?;
                return Ok(());
            } else {
                return Err(
                    format!("Erro ao enviar requisição para rota default: {}", status).into(),
                );
            }
        }
        "fallback" => {
            let fallback_request = payments_request(
                &client,
                PAYMENT_PROCESSOR_FALLBACK_URL.as_str().parse().unwrap(),
                &payload,
            )
            .await?;
            let status = fallback_request.status();
            if status.is_success() {
                store_summary(
                    &mut conn,
                    "fallback",
                    &id,
                    payment.amount,
                    timestamp_ms,
                    &payment_json,
                )
                .await?;
                return Ok(());
            } else {
                return Err(
                    format!("Erro ao enviar requisição para rota default: {}", status).into(),
                );
            }
        }
        "nenhuma" => {
            return Err(
                "[PROCESS] Nenhuma rota disponível, Colocando na fila novamente"
                    .to_string()
                    .into(),
            );
        }
        _ => {
            return Err(format!("Rota inválida: {}", rota).into());
        }
    }

    /*if rota == "default" || rota == "nenhuma" {
        //println!("Rota escolhida: {}", rota);
        for _ in 0..5 {
            let normal_request = payments_request(&client, PAYMENT_PROCESSOR_DEFAULT_URL.as_str().parse().unwrap(), &payload).await?;
            let status = normal_request.status();
            if status.is_success() {
                store_summary(&mut conn, "default", &id, payment.amount, timestamp_ms, &payment_json).await?;
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    if rota == "fallback" {
        //println!("Rota escolhida: {}", rota);
        let fallback_request = payments_request(&client, PAYMENT_PROCESSOR_FALLBACK_URL.as_str().parse().unwrap(), &payload).await?;
        let status = fallback_request.status();
        if status.is_success() {
            store_summary(&mut conn, "fallback", &id, payment.amount, timestamp_ms, &payment_json).await?;
            return Ok(());
        }
    }*/

    Err("Erro ao enviar todas as requisiçoes".to_string().into())
}

pub async fn escolher_rota_async(tolerancia: f64) -> &'static str {
    const TEMPO_RAPIDO_MINIMO_MS: i64 = 2000;
    const PENALIDADE_FALLBACK: f64 = 1.5;
    let status = GLOBAL_HEALTH_STATUS.read().await;

    let default = &status.default;
    let fallback = &status.fallback;

    if default.failing && fallback.failing {
        println!("Ambas as rotas estão falhando, nenhuma rota disponível");
        return "nenhuma";
    }

    if default.failing {
        println!("status: {:?}", status);
        println!("Rota default está falhando, usando rota fallback");
        return "fallback";
    }

    if fallback.failing {
        println!("Rota fallback está falhando, usando rota default");
        return "default";
    }

    if default.min_response_time <= TEMPO_RAPIDO_MINIMO_MS {
        println!(
            "Rota default com tempo de resposta rápido: {}ms ",
            default.min_response_time
        );
        return "default";
    }

    /*if default.min_response_time > limite_ms {
        return "fallback";
    }*/

    //let dif = default.min_response_time - fallback.min_response_time;
    //let proporcao = dif as f64 / default.min_response_time as f64;
    let mut proporcao = 0.0;
    if default.min_response_time > 0 {
        let fallback_penalizado = fallback.min_response_time as f64 * PENALIDADE_FALLBACK;
        proporcao = ((default.min_response_time as f64 - fallback_penalizado)
            / default.min_response_time as f64)
            .max(0.0);
    }
    if proporcao >= tolerancia {
        println!(
            "Rota default com proporção de sucesso maior que a tolerância, usando rota default"
        );
        "fallback"
    } else {
        println!(
            "Rota fallback com proporção de sucesso menor que a tolerância, usando rota default"
        );
        "default"
    }
}
