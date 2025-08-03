use crate::infrastructure::config::{GLOBAL_HEALTH_STATUS, PAYMENT_PROCESSOR_DEFAULT_URL, PAYMENT_PROCESSOR_FALLBACK_URL};
use crate::HealthResponse;
use reqwest::{Client};
use crate::domain::entities::ProcessorDecision;

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
                    if is_default {
                        guard.default = json;
                    } else {
                        guard.fallback = json;
                    }
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
        return ProcessorDecision::FALLBACK;
    }
    if health.fallback.failing {
        return ProcessorDecision::DEFAULT;
    }
    ProcessorDecision::DEFAULT
}
