use crate::domain::entities::ProcessorDecision;
use crate::infrastructure::config::{GLOBAL_HEALTH_STATUS, PAYMENT_PROCESSOR_DEFAULT_URL, PAYMENT_PROCESSOR_FALLBACK_URL};
use crate::HealthResponse;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn start_service_health() {
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(100))
        .build()
        .expect("failed to build health check client");

    tokio::spawn(async move {
        loop {
            check_health(&client, PAYMENT_PROCESSOR_DEFAULT_URL.to_string(), true).await;
            check_health(&client, PAYMENT_PROCESSOR_FALLBACK_URL.to_string(), false).await;

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

async fn check_health(client: &Client, base_url: String, is_default: bool) {
    let url = format!("{}/payments/service-health", base_url);
    match client.get(&url).send().await {
        Ok(response) => {
            match response.json::<HealthResponse>().await {
                Ok(json) => {
                    let mut guard = GLOBAL_HEALTH_STATUS.write().await;
                    /*if is_default {
                        guard.default = json;
                    } else {
                        guard.fallback = json;
                    }*/
                    let processor = if is_default {
                        &mut guard.default
                    } else {
                        &mut guard.fallback
                    };

                    if json.failing {
                        if processor.failing_since.is_none() {
                            processor.failing_since = Some(now_unix_ms());
                        }
                    } else {
                        processor.failing_since = None;
                    }
                    processor.failing = json.failing;
                    processor.min_response_time = json.min_response_time;

                }
                Err(e) => eprintln!("[HEALTH] Erro ao fazer parsing JSON de {}: {}", base_url, e),
            }
        }
        Err(e) => eprintln!("[HEALTH] Falha ao chamar {}: {}", base_url, e),
    }
}

pub async fn get_best_processor() -> ProcessorDecision {
    let health = GLOBAL_HEALTH_STATUS.read().await;

    if health.default.failing && health.fallback.failing {
        return ProcessorDecision::FAILING;
    }

    if health.default.failing {
        if let Some(start_ms) = health.default.failing_since {
            if let Some(elapsed) = elapsed_since(start_ms) {
                if elapsed.as_secs_f32() > 2.0 {
                    return ProcessorDecision::FALLBACK;
                }
            }
        }
        return ProcessorDecision::FAILING;
    }

    if health.fallback.failing {
        return ProcessorDecision::DEFAULT;
    }

    ProcessorDecision::DEFAULT
}


fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn elapsed_since(timestamp_ms: u64) -> Option<std::time::Duration> {
    let now = now_unix_ms();
    if now > timestamp_ms {
        Some(std::time::Duration::from_millis(now - timestamp_ms))
    } else {
        None
    }
}
