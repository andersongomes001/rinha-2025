use std::sync::Arc;
use reqwest::{Client, Response};
use serde_json::Value;

pub async fn payments_request(client: &Arc<Client>, host: String, payload: &Value) -> Result<Response, reqwest::Error> {
    client
        .post(format!("{}/payments", host))
        .json(&payload)
        .send().await
}
