use axum::{
    routing::{get, post}
    , Router,
};
use redis::aio::ConnectionManager;
use reqwest::Client;
use rinha2025::api::handlers::{clear_redis, payments, payments_summary};
use std::env;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use rinha2025::application::process;
use rinha2025::domain::entities::{AppState, PostPayments};
use rinha2025::infrastructure::redis::get_redis_connection;

#[tokio::main]
async fn main() {
    //start_service_health();
    let (tx, rx) = mpsc::channel::<PostPayments>(100_000);
    let tx_for_worker = tx.clone();
    let port = env::var("PORT").unwrap_or("9999".to_string());
    let client = Arc::new(Client::builder()
        //.timeout(Duration::from_millis(300))
        .build()
        .unwrap());

    let connection : Arc<ConnectionManager> = match get_redis_connection().await {
        Ok(conn) => Arc::new(conn),
        Err(e) => {
            eprintln!("Falha ao conectar no redis: {:?}", e);
            return;
        }
    };
    let connection_for_clear = Arc::clone(&connection);
    clear_redis(connection_for_clear).await;


    let rx = Arc::new(Mutex::new(rx));

    for _ in 0..5 {
        let connection_for_worker = Arc::clone(&connection);
        let client_clone = Arc::clone(&client);
        let rx_clone = Arc::clone(&rx);
        let tx_for_worker = tx_for_worker.clone();

        tokio::spawn(async move {
            let client = client_clone;
            let conn_clone = Arc::clone(&connection_for_worker);

            loop {
                // trava o mutex só enquanto faz o recv
                let maybe_payment = {
                    let mut rx_guard = rx_clone.lock().await;
                    rx_guard.recv().await
                };

                if let Some(post_payments) = maybe_payment {
                    let payload = serde_json::to_string(&post_payments).unwrap();
                    if let Err(e) = process(payload, conn_clone.clone(), client.clone()).await {
                        eprintln!("Erro ao processar pagamento: {:?}", e);
                        if let Err(e) = tx_for_worker.send(post_payments).await {
                            eprintln!("Erro ao tentar colocar o pagamento na fila novamente: {:?}", e);
                        } else {
                            eprintln!("Pagamento recolocado na fila.");
                        }
                    }
                } else {
                    break;
                }
            }
        });
    }

    /*
    let connection_for_worker = Arc::clone(&connection);
    tokio::spawn(async move {
        let client = Arc::clone(&client);
        let conn_clone = Arc::clone(&connection_for_worker);
        while let Some(post_payments) = rx.recv().await {
            let payload = serde_json::to_string(&post_payments).unwrap();
            if let Err(e) = process(payload, conn_clone.clone(), client.clone()).await {
                eprintln!("Erro ao processar pagamento: {:?}", e);
                if let Err(e) = tx_for_worker.send(post_payments).await {
                    eprintln!("Erro ao tentar colocar o pagamento na fila novamente: {:?}", e);
                } else {
                    eprintln!("Pagamento recolocado na fila.");
                }
            }
        }
    });
    */

    let app = Router::new()
        .route("/payments", post(payments))
        .route("/payments-summary", get(payments_summary))
        //.route("/clear_redis",get(clear_redis))
        .with_state(AppState {
            redis: Arc::clone(&connection),
            sender: tx,
        });
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

