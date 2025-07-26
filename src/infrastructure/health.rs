use crate::infrastructure::config::{GLOBAL_HEALTH_STATUS, PAYMENT_PROCESSOR_DEFAULT_URL, PAYMENT_PROCESSOR_FALLBACK_URL};
use crate::HealthResponse;
use reqwest::{Client, Error, Response};

pub fn start_service_health() {
    tokio::spawn(async move {
        loop {
            let result_default = call_health(PAYMENT_PROCESSOR_DEFAULT_URL.to_string());
            match result_default.await {
                Ok(response) => {
                    if let Ok(json) = response.json::<HealthResponse>().await {
                        GLOBAL_HEALTH_STATUS.write().await.default = json;
                    }
                }
                Err(e) => {
                    eprintln!("[HEALTH] Falha ao obter o status de default: {}", e);
                }
            }

            let result_fallback = call_health(PAYMENT_PROCESSOR_FALLBACK_URL.to_string());
            match result_fallback.await {
                Ok(response) => {
                    if let Ok(json) = response.json::<HealthResponse>().await {
                        GLOBAL_HEALTH_STATUS.write().await.fallback = json;
                    }
                }
                Err(e) => {
                    eprintln!("[HEALTH] Falha ao obter o status de fallback: {}", e);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

async fn call_health(url: String) -> Result<Response, Error> {
    let result_fallback = Client::builder()
        //.timeout(std::time::Duration::from_millis(10))
        .build()
        .unwrap()
        .get(format!("{}/payments/service-health", url))
        .send()
        .await;
    result_fallback
}

