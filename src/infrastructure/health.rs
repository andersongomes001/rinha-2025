use std::sync::atomic::Ordering;
use reqwest::Client;
use crate::HealthResponse;
use crate::infrastructure::config::{HEALTH_STATUS, PAYMENT_PROCESSOR_DEFAULT_URL};

pub fn start_service_health() {
    tokio::spawn(async move {
        loop {
            let result = Client::builder()
                .timeout(std::time::Duration::from_millis(10))
                .build()
                .unwrap()
                .get(format!("{}/payments/service-health",PAYMENT_PROCESSOR_DEFAULT_URL.to_string()))
                .send()
                .await;
            match result {
                Ok(response) => {
                    if let Ok(json) = response.json::<HealthResponse>().await {
                        HEALTH_STATUS.store(!json.failing, Ordering::Relaxed);
                    } else {
                        HEALTH_STATUS.store(false, Ordering::Relaxed);
                    }

                }
                Err(_) => {
                    HEALTH_STATUS.store(false, Ordering::Relaxed);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}
