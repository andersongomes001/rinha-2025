use axum::{
    routing::{get, post}
    , Router,
};
use redis::aio::ConnectionManager;
use reqwest::Client;
use rinha2025::api::handlers::{payments, payments_summary};
use rinha2025::application::process;
use rinha2025::domain::entities::{AppState, PostPayments, ProcessorDecision};
use rinha2025::infrastructure::config::INSTANCE_ROLE;
use rinha2025::infrastructure::health::{get_best_processor, start_service_health};
use rinha2025::infrastructure::redis::get_redis_connection;
use rinha2025::infrastructure::{run_master, run_slave};
use std::env;
use std::sync::Arc;
use axum::body::{Bytes};
use tokio::sync::{mpsc, Semaphore};

#[tokio::main]
async fn main() {
    /*tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();*/

    if INSTANCE_ROLE.as_str() == "master" {
        start_service_health();
        run_master().await;
    }
    if INSTANCE_ROLE.as_str() == "slave" {
        run_slave().await;
    }
    let workers = std::env::var("MAX_WORKERS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(2);

    let (tx, mut rx) = mpsc::channel::<Bytes>(
        std::env::var("CHANNEL_CAPACITY").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(4096)
    );
    let tx_for_worker = tx.clone();
    let port = env::var("PORT").unwrap_or("9999".to_string());
    let client = Arc::new(Client::builder()
        .connect_timeout(std::time::Duration::from_millis(75))
        .timeout(std::time::Duration::from_millis(250))
        .pool_idle_timeout(std::time::Duration::from_secs(5))
        .tcp_nodelay(true)
        .build()
        .unwrap());

    let connection : Arc<ConnectionManager> = match get_redis_connection().await {
        Ok(conn) => Arc::new(conn),
        Err(e) => {
            eprintln!("Falha ao conectar no redis: {:?}", e);
            return;
        }
    };


    let semaphore = Arc::new(Semaphore::new(workers));

    let connection_for_tasks = Arc::clone(&connection);
    let client_for_tasks = Arc::clone(&client);
    let tx_for_requeue = tx_for_worker.clone();

    tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let conn_clone = Arc::clone(&connection_for_tasks);
            let client_clone = Arc::clone(&client_for_tasks);
            let tx_clone = tx_for_requeue.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Ok(post_payments) = serde_json::from_slice::<PostPayments>(&bytes) {
                    let payload = serde_json::to_string(&post_payments).unwrap();
                    let decision = get_best_processor().await;
                    if let Err(e) = process(payload, conn_clone.clone(), client_clone.clone(), decision).await {
                        let _ = tx_clone.try_send(bytes);
                        eprintln!("Erro ao processar pagamento: {:?}", e);
                    }
                }
            });
        }
    });


    /*
    let connection_for_worker = Arc::clone(&connection);
    tokio::spawn(async move {
        let client = Arc::clone(&client);
        let conn_clone = Arc::clone(&connection_for_worker);
        while let Some(bytes) = rx.recv().await {
            if let Ok(post_payments) = serde_json::from_slice::<PostPayments>(&bytes) {
                let decision = get_best_processor().await;
                if decision == ProcessorDecision::FAILING {
                    eprintln!("Processor em estado FAILING. Aguardando...");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
                let payload = serde_json::to_string(&post_payments).unwrap();
                if let Err(e) = process(payload, conn_clone.clone(), client.clone(), decision).await {
                    eprintln!("Erro ao processar pagamento: {:?}", e);
                    if let Err(e) = tx_for_worker.send(bytes) {
                        eprintln!("Erro ao tentar colocar o pagamento na fila novamente: {:?}", e);
                    } else {
                        eprintln!("Pagamento recolocado na fila.");
                    }
                }
            }
        }
    });
    */

    let app = Router::new()
        .route("/payments", post(payments))
        .route("/payments-summary", get(payments_summary))
        /*.layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )*/
        .with_state(AppState {
            redis: Arc::clone(&connection),
            sender: tx
        });
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

